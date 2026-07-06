use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mir2_game_data::crystal_monster_by_name;
use mir2_protocol::{
    ClientPacket, MirDirection, MonsterInfo, NpcInfo, ObjectDiedInfo, ObjectGoldInfo,
    ObjectHealthInfo, ObjectItemInfo, ObjectPlayerInfo, Point, ServerPacket, Spell,
};
use mir2_simulation::{
    intelligent_creature_allows_ground_drop, zone_ground_drop_snapshots_for_monster_at_tick,
    ActiveSessionIdentity, ChatPacketPreparation, GroundDropLootSnapshot, GroundDropSnapshot,
    InProcessWorldRuntime, SessionId, SharedAccountInventoryTransactionKind,
    SharedAccountInventoryTransactionReceipt, SharedItemRentalAgreement, SharedItemRentalDelivery,
    SharedItemRentalFeeOffer, SharedItemRentalItemOffer, SharedNpcSavedValue, SharedTradeOffer,
    WorldCommand, WorldCommandExecution, WorldEntityDisposition, WorldEntityKind,
    WorldEntitySnapshot, WorldEntitySpriteSnapshot, WorldRuntime, WorldSnapshot, ZoneCommand,
    ZoneManager, ZoneMonsterDefense, ZoneMonsterKillAward, ZoneMonsterSpawn, ZoneOutbound,
    ZoneRuntimeHandle,
};

use crate::GatewayConfig;

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedTickProbeMetrics {
    pub(crate) count: u64,
    pub(crate) pending: SharedTickProbePhaseMetrics,
    pub(crate) expire: SharedTickProbePhaseMetrics,
    pub(crate) command: SharedTickProbePhaseMetrics,
    pub(crate) auto_pickup: SharedTickProbePhaseMetrics,
    pub(crate) observer: SharedTickProbePhaseMetrics,
    pub(crate) observer_detail: SharedTickProbeObserverMetrics,
    pub(crate) zone: SharedTickProbePhaseMetrics,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedTickProbePhaseMetrics {
    pub(crate) count: u64,
    pub(crate) ms_total: u64,
    pub(crate) ms_last: u64,
    pub(crate) ms_max: u64,
    pub(crate) ms_avg: u64,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedTickProbeObserverMetrics {
    pub(crate) npc: SharedTickProbePhaseMetrics,
    pub(crate) apply: SharedTickProbePhaseMetrics,
    pub(crate) shared: SharedTickProbePhaseMetrics,
    pub(crate) drops: SharedTickProbePhaseMetrics,
    pub(crate) quest: SharedTickProbePhaseMetrics,
    pub(crate) zone_observer: SharedTickProbePhaseMetrics,
}

struct SharedTickProbePhase {
    total: AtomicU64,
    last: AtomicU64,
    max: AtomicU64,
}

impl SharedTickProbePhase {
    const fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            last: AtomicU64::new(0),
            max: AtomicU64::new(0),
        }
    }

    fn record(&self, ms: u64) {
        self.total.fetch_add(ms, Ordering::Relaxed);
        self.last.store(ms, Ordering::Relaxed);
        let mut current_max = self.max.load(Ordering::Relaxed);
        while ms > current_max {
            match self
                .max
                .compare_exchange(current_max, ms, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
    }

    fn summary(&self, count: u64) -> SharedTickProbePhaseMetrics {
        let total = self.total.load(Ordering::Relaxed);
        SharedTickProbePhaseMetrics {
            count,
            ms_total: total,
            ms_last: self.last.load(Ordering::Relaxed),
            ms_max: self.max.load(Ordering::Relaxed),
            ms_avg: if count > 0 { total / count } else { 0 },
        }
    }
}

static SHARED_TICK_PROBE_COUNT: AtomicU64 = AtomicU64::new(0);
static SHARED_TICK_PENDING_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_EXPIRE_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_COMMAND_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_AUTO_PICKUP_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_NPC_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_APPLY_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_SHARED_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_DROPS_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_QUEST_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_OBSERVER_ZONE_MS: SharedTickProbePhase = SharedTickProbePhase::new();
static SHARED_TICK_ZONE_MS: SharedTickProbePhase = SharedTickProbePhase::new();

fn probe_elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u64::MAX as u128) as u64
}

fn shared_tick_probe_phase_sample(ms: u64) -> SharedTickProbePhaseMetrics {
    SharedTickProbePhaseMetrics {
        count: 0,
        ms_total: 0,
        ms_last: ms,
        ms_max: 0,
        ms_avg: 0,
    }
}

fn shared_tick_probe_observer_sample(
    npc_ms: u64,
    apply_ms: u64,
    shared_ms: u64,
    drops_ms: u64,
    quest_ms: u64,
    zone_observer_ms: u64,
) -> SharedTickProbeObserverMetrics {
    SharedTickProbeObserverMetrics {
        npc: shared_tick_probe_phase_sample(npc_ms),
        apply: shared_tick_probe_phase_sample(apply_ms),
        shared: shared_tick_probe_phase_sample(shared_ms),
        drops: shared_tick_probe_phase_sample(drops_ms),
        quest: shared_tick_probe_phase_sample(quest_ms),
        zone_observer: shared_tick_probe_phase_sample(zone_observer_ms),
    }
}

fn record_shared_tick_probe(sample: SharedTickProbeMetrics) {
    SHARED_TICK_PENDING_MS.record(sample.pending.ms_last);
    SHARED_TICK_EXPIRE_MS.record(sample.expire.ms_last);
    SHARED_TICK_COMMAND_MS.record(sample.command.ms_last);
    SHARED_TICK_AUTO_PICKUP_MS.record(sample.auto_pickup.ms_last);
    SHARED_TICK_OBSERVER_MS.record(sample.observer.ms_last);
    SHARED_TICK_OBSERVER_NPC_MS.record(sample.observer_detail.npc.ms_last);
    SHARED_TICK_OBSERVER_APPLY_MS.record(sample.observer_detail.apply.ms_last);
    SHARED_TICK_OBSERVER_SHARED_MS.record(sample.observer_detail.shared.ms_last);
    SHARED_TICK_OBSERVER_DROPS_MS.record(sample.observer_detail.drops.ms_last);
    SHARED_TICK_OBSERVER_QUEST_MS.record(sample.observer_detail.quest.ms_last);
    SHARED_TICK_OBSERVER_ZONE_MS.record(sample.observer_detail.zone_observer.ms_last);
    SHARED_TICK_ZONE_MS.record(sample.zone.ms_last);
    SHARED_TICK_PROBE_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn shared_tick_probe_metrics() -> SharedTickProbeMetrics {
    let count = SHARED_TICK_PROBE_COUNT.load(Ordering::Relaxed);
    SharedTickProbeMetrics {
        count,
        pending: SHARED_TICK_PENDING_MS.summary(count),
        expire: SHARED_TICK_EXPIRE_MS.summary(count),
        command: SHARED_TICK_COMMAND_MS.summary(count),
        auto_pickup: SHARED_TICK_AUTO_PICKUP_MS.summary(count),
        observer: SHARED_TICK_OBSERVER_MS.summary(count),
        observer_detail: SharedTickProbeObserverMetrics {
            npc: SHARED_TICK_OBSERVER_NPC_MS.summary(count),
            apply: SHARED_TICK_OBSERVER_APPLY_MS.summary(count),
            shared: SHARED_TICK_OBSERVER_SHARED_MS.summary(count),
            drops: SHARED_TICK_OBSERVER_DROPS_MS.summary(count),
            quest: SHARED_TICK_OBSERVER_QUEST_MS.summary(count),
            zone_observer: SHARED_TICK_OBSERVER_ZONE_MS.summary(count),
        },
        zone: SHARED_TICK_ZONE_MS.summary(count),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZoneId(String);

impl ZoneId {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.trim().is_empty(), "zone id must not be empty");
        Self(value)
    }

    pub fn primary() -> Self {
        Self::new("primary")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ZoneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneOwnerLease {
    zone_id: ZoneId,
    owner_id: String,
    fencing_token: u64,
}

impl ZoneOwnerLease {
    pub fn new(zone_id: ZoneId, owner_id: impl Into<String>, fencing_token: u64) -> Self {
        let owner_id = owner_id.into();
        assert!(
            !owner_id.trim().is_empty(),
            "zone owner id must not be empty"
        );
        assert!(
            fencing_token > 0,
            "zone owner fencing token must be positive"
        );
        Self {
            zone_id,
            owner_id,
            fencing_token,
        }
    }

    pub fn in_process(zone_id: &ZoneId) -> Self {
        Self::new(
            zone_id.clone(),
            format!("in-process:{}", zone_id.as_str()),
            1,
        )
    }

    pub fn zone_id(&self) -> &ZoneId {
        &self.zone_id
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn fencing_token(&self) -> u64 {
        self.fencing_token
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneOwnerCommandMode {
    Direct,
    ProductionPlayer { authenticated: bool },
}

#[derive(Debug)]
pub struct ZoneOwnerCommandRequest {
    owner_lease: ZoneOwnerLease,
    mode: ZoneOwnerCommandMode,
    command: WorldCommand,
}

impl ZoneOwnerCommandRequest {
    pub fn direct(owner_lease: ZoneOwnerLease, command: WorldCommand) -> Self {
        Self {
            owner_lease,
            mode: ZoneOwnerCommandMode::Direct,
            command,
        }
    }

    pub fn production_player(
        owner_lease: ZoneOwnerLease,
        authenticated: bool,
        command: WorldCommand,
    ) -> Self {
        Self {
            owner_lease,
            mode: ZoneOwnerCommandMode::ProductionPlayer { authenticated },
            command,
        }
    }

    pub fn owner_lease(&self) -> &ZoneOwnerLease {
        &self.owner_lease
    }

    pub fn mode(&self) -> ZoneOwnerCommandMode {
        self.mode
    }

    pub fn into_command(self) -> WorldCommand {
        self.command
    }
}

pub trait ZoneOwnerLeaseAuthority: Send + Sync {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease;

    fn validate_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        let current = self.owner_lease(lease.zone_id());
        if current == *lease {
            return Ok(());
        }

        Err(stale_zone_owner_lease_error(lease, &current))
    }

    fn renew_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<ZoneOwnerLease, String> {
        self.validate_owner_lease(lease)?;
        Ok(self.owner_lease(lease.zone_id()))
    }

    fn renew_owner_lease_at(
        &self,
        lease: &ZoneOwnerLease,
        _now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        self.renew_owner_lease(lease)
    }
}

pub type SharedZoneOwnerLeaseAuthority = Arc<dyn ZoneOwnerLeaseAuthority>;

pub trait ZoneOwnerCommandClient: fmt::Debug + Send + Sync {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String>;

    fn world_snapshot(&self, runtime: &ZoneRuntimeHandle) -> Result<WorldSnapshot, String> {
        Ok(runtime.world_snapshot())
    }

    fn active_identity(
        &self,
        runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<ActiveSessionIdentity>, String> {
        Ok(runtime.active_identity())
    }

    fn save_active_character(&self, runtime: &ZoneRuntimeHandle) -> Result<(), String> {
        runtime.save_active_character();
        Ok(())
    }

    fn refresh_active_external_mail(
        &self,
        runtime: &mut ZoneRuntimeHandle,
    ) -> Result<bool, String> {
        Ok(runtime.refresh_active_external_mail())
    }
}

pub type SharedZoneOwnerCommandClient = Arc<dyn ZoneOwnerCommandClient>;

pub trait ZoneOwnerRpcTransport: fmt::Debug + Send + Sync {
    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String>;

    fn world_snapshot(&self) -> Result<WorldSnapshot, String>;

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String>;

    fn save_active_character(&self) -> Result<(), String>;

    fn refresh_active_external_mail(&self) -> Result<bool, String>;
}

pub type SharedZoneOwnerRpcTransport = Arc<dyn ZoneOwnerRpcTransport>;

#[derive(Clone)]
pub struct RpcZoneOwnerCommandClient {
    transport: SharedZoneOwnerRpcTransport,
}

impl fmt::Debug for RpcZoneOwnerCommandClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RpcZoneOwnerCommandClient")
            .field("transport", &"ZoneOwnerRpcTransport")
            .finish()
    }
}

impl RpcZoneOwnerCommandClient {
    pub fn new(transport: SharedZoneOwnerRpcTransport) -> Self {
        Self { transport }
    }
}

impl ZoneOwnerCommandClient for RpcZoneOwnerCommandClient {
    fn execute(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.transport.execute(request)
    }

    fn world_snapshot(&self, _runtime: &ZoneRuntimeHandle) -> Result<WorldSnapshot, String> {
        self.transport.world_snapshot()
    }

    fn active_identity(
        &self,
        _runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<ActiveSessionIdentity>, String> {
        self.transport.active_identity()
    }

    fn save_active_character(&self, _runtime: &ZoneRuntimeHandle) -> Result<(), String> {
        self.transport.save_active_character()
    }

    fn refresh_active_external_mail(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
    ) -> Result<bool, String> {
        self.transport.refresh_active_external_mail()
    }
}

fn stale_zone_owner_lease_error(lease: &ZoneOwnerLease, current: &ZoneOwnerLease) -> String {
    format!(
        "stale zone owner lease for zone {}: current owner {} fencing token {}, got owner {} fencing token {}",
        lease.zone_id(),
        current.owner_id(),
        current.fencing_token(),
        lease.owner_id(),
        lease.fencing_token()
    )
}

#[derive(Default)]
pub struct InProcessZoneOwnerCommandClient {
    owner_lease_authority: Option<SharedZoneOwnerLeaseAuthority>,
}

impl fmt::Debug for InProcessZoneOwnerCommandClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InProcessZoneOwnerCommandClient")
            .field(
                "owner_lease_authority",
                &self
                    .owner_lease_authority
                    .as_ref()
                    .map(|_| "ZoneOwnerLeaseAuthority"),
            )
            .finish()
    }
}

impl InProcessZoneOwnerCommandClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_owner_lease_authority(
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        Self {
            owner_lease_authority: Some(owner_lease_authority),
        }
    }
}

impl ZoneOwnerCommandClient for InProcessZoneOwnerCommandClient {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(request.owner_lease())?;
        }
        let mode = request.mode();
        let command = request.into_command();
        match mode {
            ZoneOwnerCommandMode::Direct => runtime.execute_with_outcome(command),
            ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                runtime.execute_production_player_command(authenticated, command)
            }
        }
    }
}

pub struct HostedZoneOwnerCommandClient {
    runtime: Mutex<Option<ZoneRuntimeHandle>>,
    owner_lease_authority: Option<SharedZoneOwnerLeaseAuthority>,
}

impl fmt::Debug for HostedZoneOwnerCommandClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostedZoneOwnerCommandClient")
            .field("runtime", &"WorldRuntime")
            .field(
                "owner_lease_authority",
                &self
                    .owner_lease_authority
                    .as_ref()
                    .map(|_| "ZoneOwnerLeaseAuthority"),
            )
            .finish()
    }
}

impl HostedZoneOwnerCommandClient {
    pub fn new(runtime: ZoneRuntimeHandle) -> Self {
        Self {
            runtime: Mutex::new(Some(runtime)),
            owner_lease_authority: None,
        }
    }

    pub fn with_owner_lease_authority(
        runtime: ZoneRuntimeHandle,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        Self {
            runtime: Mutex::new(Some(runtime)),
            owner_lease_authority: Some(owner_lease_authority),
        }
    }

    pub fn from_handoff(runtime: ZoneRuntimeHandle) -> Self {
        Self::new(runtime)
    }

    pub fn from_handoff_with_owner_lease_authority(
        runtime: ZoneRuntimeHandle,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        Self::with_owner_lease_authority(runtime, owner_lease_authority)
    }

    pub fn take_runtime_for_handoff(&self) -> Result<ZoneRuntimeHandle, String> {
        self.runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?
            .take()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }

    pub fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_ref()
            .map(|runtime| runtime.world_snapshot())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }

    pub fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_ref()
            .map(|runtime| runtime.active_identity())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }

    pub fn execute_request(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(request.owner_lease())?;
        }
        let mode = request.mode();
        let command = request.into_command();
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let Some(runtime) = runtime.as_mut() else {
            return Err("zone owner hosted runtime was already handed off".to_string());
        };
        match mode {
            ZoneOwnerCommandMode::Direct => runtime.execute_with_outcome(command),
            ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                runtime.execute_production_player_command(authenticated, command)
            }
        }
    }
}

impl ZoneOwnerCommandClient for HostedZoneOwnerCommandClient {
    fn execute(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.execute_request(request)
    }

    fn world_snapshot(&self, _runtime: &ZoneRuntimeHandle) -> Result<WorldSnapshot, String> {
        HostedZoneOwnerCommandClient::world_snapshot(self)
    }

    fn active_identity(
        &self,
        _runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<ActiveSessionIdentity>, String> {
        HostedZoneOwnerCommandClient::active_identity(self)
    }

    fn save_active_character(&self, _runtime: &ZoneRuntimeHandle) -> Result<(), String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_ref()
            .map(|runtime| runtime.save_active_character())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }

    fn refresh_active_external_mail(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
    ) -> Result<bool, String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_mut()
            .map(|runtime| runtime.refresh_active_external_mail())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }
}

impl ZoneOwnerRpcTransport for HostedZoneOwnerCommandClient {
    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.execute_request(request)
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        HostedZoneOwnerCommandClient::world_snapshot(self)
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        HostedZoneOwnerCommandClient::active_identity(self)
    }

    fn save_active_character(&self) -> Result<(), String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_ref()
            .map(|runtime| runtime.save_active_character())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }

    fn refresh_active_external_mail(&self) -> Result<bool, String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_mut()
            .map(|runtime| runtime.refresh_active_external_mail())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }
}

#[derive(Debug, Clone)]
struct ZoneOwnerLeaseRecord {
    lease: ZoneOwnerLease,
    expires_at_ms: Option<u64>,
}

impl ZoneOwnerLeaseRecord {
    fn new(lease: ZoneOwnerLease, expires_at_ms: Option<u64>) -> Self {
        Self {
            lease,
            expires_at_ms,
        }
    }

    fn expired_at(&self, now_ms: u64) -> bool {
        self.expires_at_ms
            .is_some_and(|expires_at_ms| now_ms >= expires_at_ms)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryZoneOwnerLeaseAuthority {
    leases: Mutex<BTreeMap<ZoneId, ZoneOwnerLeaseRecord>>,
    lease_ttl_ms: Option<u64>,
}

impl InMemoryZoneOwnerLeaseAuthority {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_lease_ttl_ms(lease_ttl_ms: u64) -> Self {
        Self {
            leases: Mutex::new(BTreeMap::new()),
            lease_ttl_ms: Some(lease_ttl_ms.max(1)),
        }
    }

    pub fn handoff_zone_owner(
        &self,
        zone_id: &ZoneId,
        owner_id: impl Into<String>,
    ) -> ZoneOwnerLease {
        self.handoff_zone_owner_at(zone_id, owner_id, shared_gateway_now_ms())
    }

    pub fn handoff_zone_owner_at(
        &self,
        zone_id: &ZoneId,
        owner_id: impl Into<String>,
        now_ms: u64,
    ) -> ZoneOwnerLease {
        let mut leases = self
            .leases
            .lock()
            .expect("zone owner lease mutex should not be poisoned");
        let fencing_token = leases
            .get(zone_id)
            .map(|record| record.lease.fencing_token().saturating_add(1))
            .unwrap_or(1);
        let lease = ZoneOwnerLease::new(zone_id.clone(), owner_id, fencing_token);
        leases.insert(
            zone_id.clone(),
            ZoneOwnerLeaseRecord::new(lease.clone(), self.expires_at_ms(now_ms)),
        );
        lease
    }

    pub fn owner_lease_at(&self, zone_id: &ZoneId, now_ms: u64) -> ZoneOwnerLease {
        let mut leases = self
            .leases
            .lock()
            .expect("zone owner lease mutex should not be poisoned");
        if let Some(record) = leases.get(zone_id) {
            if !record.expired_at(now_ms) {
                return record.lease.clone();
            }
        }
        let fencing_token = leases
            .get(zone_id)
            .map(|record| record.lease.fencing_token().saturating_add(1))
            .unwrap_or(1);
        let lease = ZoneOwnerLease::new(
            zone_id.clone(),
            format!("in-process:{}", zone_id.as_str()),
            fencing_token,
        );
        leases.insert(
            zone_id.clone(),
            ZoneOwnerLeaseRecord::new(lease.clone(), self.expires_at_ms(now_ms)),
        );
        lease
    }

    pub fn renew_owner_lease_at(
        &self,
        lease: &ZoneOwnerLease,
        now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        let current = self.owner_lease_at(lease.zone_id(), now_ms);
        if current != *lease {
            return Err(stale_zone_owner_lease_error(lease, &current));
        }
        let mut leases = self
            .leases
            .lock()
            .expect("zone owner lease mutex should not be poisoned");
        leases.insert(
            lease.zone_id().clone(),
            ZoneOwnerLeaseRecord::new(lease.clone(), self.expires_at_ms(now_ms)),
        );
        Ok(lease.clone())
    }

    fn expires_at_ms(&self, now_ms: u64) -> Option<u64> {
        self.lease_ttl_ms
            .map(|lease_ttl_ms| now_ms.saturating_add(lease_ttl_ms))
    }
}

impl ZoneOwnerLeaseAuthority for InMemoryZoneOwnerLeaseAuthority {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.owner_lease_at(zone_id, shared_gateway_now_ms())
    }

    fn renew_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<ZoneOwnerLease, String> {
        self.renew_owner_lease_at(lease, shared_gateway_now_ms())
    }

    fn renew_owner_lease_at(
        &self,
        lease: &ZoneOwnerLease,
        now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        InMemoryZoneOwnerLeaseAuthority::renew_owner_lease_at(self, lease, now_ms)
    }
}

pub trait ZoneRuntimeFactory: Send + Sync {
    fn create_runtime(&self, config: GatewayConfig, zone_id: &ZoneId) -> ZoneRuntimeHandle;
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        ZoneOwnerLease::in_process(zone_id)
    }
}

pub type SharedZoneRuntimeFactory = Arc<dyn ZoneRuntimeFactory>;

#[derive(Debug, Clone, PartialEq)]
pub enum SharedAccountInventoryCommand {
    GroundDropPickup(GroundDropSnapshot),
    MonsterKillAward(ZoneMonsterKillAward),
    SkillItemConsume { spell: Spell, request_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SharedAccountInventoryCommandKey(String);

#[derive(Debug, Clone, PartialEq)]
pub struct SharedAccountInventoryCommandEnvelope {
    pub identity: ActiveSessionIdentity,
    pub command: SharedAccountInventoryCommand,
}

impl SharedAccountInventoryCommandEnvelope {
    fn idempotency_key(&self) -> Option<SharedAccountInventoryCommandKey> {
        let command_key = match &self.command {
            SharedAccountInventoryCommand::GroundDropPickup(drop) => {
                format!("ground-drop-pickup:{}", drop.object_id)
            }
            SharedAccountInventoryCommand::MonsterKillAward(award) => format!(
                "monster-kill-award:{}:{}:{}",
                award.monster_object_id, award.monster_name, award.experience
            ),
            SharedAccountInventoryCommand::SkillItemConsume { spell, request_id } => {
                format!("skill-item-consume:{}:{}", *spell as u8, request_id)
            }
        };
        Some(SharedAccountInventoryCommandKey(format!(
            "{}:{}:{}",
            self.identity.account_id, self.identity.character_index, command_key
        )))
    }
}

pub trait SharedAccountInventoryService: fmt::Debug + Send + Sync {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> mir2_simulation::SharedAccountInventoryTransactionReceipt;

    fn commit_ground_drop_pickup(
        &self,
        runtime: &mut InProcessWorldRuntime,
        drop: &GroundDropSnapshot,
    ) -> mir2_simulation::SharedAccountInventoryTransactionReceipt {
        self.commit(
            runtime,
            SharedAccountInventoryCommandEnvelope {
                identity: runtime
                    .active_identity()
                    .expect("shared account/inventory pickup requires an active character"),
                command: SharedAccountInventoryCommand::GroundDropPickup(drop.clone()),
            },
        )
    }

    fn commit_monster_kill_award(
        &self,
        runtime: &mut InProcessWorldRuntime,
        award: &ZoneMonsterKillAward,
    ) -> mir2_simulation::SharedAccountInventoryTransactionReceipt {
        self.commit(
            runtime,
            SharedAccountInventoryCommandEnvelope {
                identity: runtime
                    .active_identity()
                    .expect("shared account/inventory award requires an active character"),
                command: SharedAccountInventoryCommand::MonsterKillAward(award.clone()),
            },
        )
    }
}

pub type SharedAccountInventoryServiceHandle = Arc<dyn SharedAccountInventoryService>;

#[derive(Debug, Default)]
pub struct InProcessAccountInventoryService {
    committed_receipts:
        Mutex<BTreeMap<SharedAccountInventoryCommandKey, SharedAccountInventoryTransactionReceipt>>,
}

impl InProcessAccountInventoryService {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SharedAccountInventoryService for InProcessAccountInventoryService {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        let idempotency_key = envelope.idempotency_key();
        if let Some(key) = idempotency_key.as_ref() {
            if let Some(receipt) = self
                .committed_receipts
                .lock()
                .expect("account inventory idempotency mutex should not be poisoned")
                .get(key)
                .cloned()
            {
                return receipt;
            }
        }
        let kind = match &envelope.command {
            SharedAccountInventoryCommand::GroundDropPickup(_) => {
                SharedAccountInventoryTransactionKind::GroundDropPickup
            }
            SharedAccountInventoryCommand::MonsterKillAward(_) => {
                SharedAccountInventoryTransactionKind::MonsterKillAward
            }
            SharedAccountInventoryCommand::SkillItemConsume { .. } => {
                SharedAccountInventoryTransactionKind::SkillItemConsumption
            }
        };
        if runtime.active_identity().as_ref() != Some(&envelope.identity) {
            return SharedAccountInventoryTransactionReceipt {
                kind,
                committed: false,
                packets: Vec::new(),
            };
        }
        let receipt = match envelope.command {
            SharedAccountInventoryCommand::GroundDropPickup(drop) => {
                runtime.commit_shared_ground_drop_pickup_transaction(&drop)
            }
            SharedAccountInventoryCommand::MonsterKillAward(award) => runtime
                .commit_shared_monster_kill_award_transaction(
                    award.monster_object_id,
                    &award.monster_name,
                    award.experience,
                ),
            SharedAccountInventoryCommand::SkillItemConsume { spell, .. } => {
                runtime.commit_shared_skill_item_consumption_transaction(spell)
            }
        };
        if let Some(key) = idempotency_key {
            if receipt.committed {
                self.committed_receipts
                    .lock()
                    .expect("account inventory idempotency mutex should not be poisoned")
                    .insert(key, receipt.clone());
            }
        }
        receipt
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedNpcEntitySideEffect {
    pub map_file_name: String,
    pub packets: Vec<ServerPacket>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SharedNpcWorldCommand {
    SyncSavedValues(Vec<SharedNpcSavedValue>),
    SyncRandomSeed(u64),
    ApplyEntitySideEffects {
        map_file_name: String,
        packets: Vec<ServerPacket>,
    },
    ApplyScriptOutcome {
        saved_values: Vec<SharedNpcSavedValue>,
        random_seed: u64,
        entity_side_effect: Option<SharedNpcEntitySideEffect>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedNpcWorldCommandEnvelope {
    pub identity: ActiveSessionIdentity,
    pub command: SharedNpcWorldCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedNpcWorldTransactionReceipt {
    pub committed: bool,
    pub command: SharedNpcWorldCommand,
}

impl SharedNpcWorldTransactionReceipt {
    pub fn committed(command: SharedNpcWorldCommand) -> Self {
        Self {
            committed: true,
            command,
        }
    }

    pub fn rejected(command: SharedNpcWorldCommand) -> Self {
        Self {
            committed: false,
            command,
        }
    }
}

pub trait SharedNpcWorldService: fmt::Debug + Send + Sync {
    fn commit(&self, envelope: SharedNpcWorldCommandEnvelope) -> SharedNpcWorldTransactionReceipt;
}

pub type SharedNpcWorldServiceHandle = Arc<dyn SharedNpcWorldService>;

#[derive(Debug, Default)]
pub struct InProcessNpcWorldService;

impl SharedNpcWorldService for InProcessNpcWorldService {
    fn commit(&self, envelope: SharedNpcWorldCommandEnvelope) -> SharedNpcWorldTransactionReceipt {
        SharedNpcWorldTransactionReceipt::committed(envelope.command)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRouteRequest {
    pub account_id: Option<String>,
    pub character_index: Option<i32>,
    pub map_file_name: Option<String>,
}

impl SessionRouteRequest {
    pub fn anonymous() -> Self {
        Self::default()
    }
}

pub trait SessionRouter: Send + Sync {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId;
}

pub type SharedSessionRouter = Arc<dyn SessionRouter>;

#[derive(Debug, Default)]
pub struct SingleZoneSessionRouter;

impl SessionRouter for SingleZoneSessionRouter {
    fn route_session(&self, _request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
        default_zone_id.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapZoneSessionRouter {
    map_routes: BTreeMap<String, ZoneId>,
}

impl MapZoneSessionRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_route(mut self, map_file_name: impl Into<String>, zone_id: ZoneId) -> Self {
        self.map_routes.insert(map_file_name.into(), zone_id);
        self
    }
}

impl SessionRouter for MapZoneSessionRouter {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
        request
            .map_file_name
            .as_ref()
            .and_then(|map_file_name| self.map_routes.get(map_file_name))
            .cloned()
            .unwrap_or_else(|| default_zone_id.clone())
    }
}

/// Routes each map to its **own** zone automatically (`ZoneId = "map:<map>"`),
/// without enumerating every map — the routing primitive for map=zone (see
/// `docs/GATEWAY-MAP-ZONE-ROUTING-DESIGN.md`). Explicit `group(map, zone)`
/// overrides let several low-traffic maps share one zone. Sessions with no map
/// (anonymous / pre-character-select) fall back to the default zone.
///
/// NOT yet wired into the live gateway: production still opens sessions
/// anonymously on the default zone. Turning this on additionally requires the
/// per-zone tick driver and the map-transfer zone handoff (design steps 2–3),
/// because routing alone would strand a player in their old zone after a
/// cross-map transfer.
#[derive(Debug, Clone, Default)]
pub struct PerMapSessionRouter {
    overrides: BTreeMap<String, ZoneId>,
}

impl PerMapSessionRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pin an explicit map into a shared/override zone (e.g. group many cold
    /// leveling maps, or give a sieged map a dedicated zone).
    pub fn group(mut self, map_file_name: impl Into<String>, zone_id: ZoneId) -> Self {
        self.overrides.insert(map_file_name.into(), zone_id);
        self
    }

    /// The default zone id a map routes to under per-map routing.
    pub fn zone_for_map(map_file_name: &str) -> ZoneId {
        ZoneId::new(format!("map:{map_file_name}"))
    }
}

impl SessionRouter for PerMapSessionRouter {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
        match request.map_file_name.as_ref() {
            Some(map_file_name) => self
                .overrides
                .get(map_file_name)
                .cloned()
                .unwrap_or_else(|| Self::zone_for_map(map_file_name)),
            None => default_zone_id.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct InProcessZoneRuntimeFactory;

impl ZoneRuntimeFactory for InProcessZoneRuntimeFactory {
    fn create_runtime(&self, config: GatewayConfig, _zone_id: &ZoneId) -> ZoneRuntimeHandle {
        Box::new(InProcessWorldRuntime::new(config))
    }
}

#[derive(Debug, Clone)]
struct ZonePresenceKey {
    account_id: String,
    character_index: i32,
}

impl ZonePresenceKey {
    fn from_identity(identity: &ActiveSessionIdentity) -> Self {
        Self {
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
        }
    }
}

impl PartialEq for ZonePresenceKey {
    fn eq(&self, other: &Self) -> bool {
        self.account_id == other.account_id && self.character_index == other.character_index
    }
}

impl Eq for ZonePresenceKey {}

impl PartialOrd for ZonePresenceKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ZonePresenceKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.account_id
            .cmp(&other.account_id)
            .then(self.character_index.cmp(&other.character_index))
    }
}

#[derive(Debug, Clone)]
struct ZonePlayerPresence {
    zone_object_id: u32,
    map_file_name: String,
    entity: WorldEntitySnapshot,
    free_bag_slots: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedDeathDropAnchor {
    monster_name: Option<String>,
    location: Point,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedDeadEntityState {
    location: Option<Point>,
    direction: Option<MirDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SharedNpcSavedValueKey {
    file_name: String,
    group: String,
    key: String,
}

impl SharedNpcSavedValueKey {
    fn from_value(value: &SharedNpcSavedValue) -> Self {
        Self {
            file_name: value.file_name.to_ascii_lowercase(),
            group: value.group.to_ascii_lowercase(),
            key: value.key.to_ascii_lowercase(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ZoneMapSnapshotLayer {
    entities: BTreeMap<u32, WorldEntitySnapshot>,
    removed_entity_ids: BTreeSet<u32>,
    dead_entity_ids: BTreeMap<u32, SharedDeadEntityState>,
    committed_death_drop_anchors: BTreeMap<u32, SharedDeathDropAnchor>,
    harvested_entity_ids: BTreeSet<u32>,
    revived_entity_ids: BTreeSet<u32>,
    ground_drops: BTreeMap<u32, GroundDropSnapshot>,
    removed_drop_ids: BTreeSet<u32>,
    drop_ownership_expires_at_ms: BTreeMap<u32, u64>,
    drop_expires_at_ms: BTreeMap<u32, u64>,
}

#[derive(Debug, Clone)]
struct SharedItemRentalInvite {
    partner_name: String,
    renting: bool,
}

#[derive(Debug)]
struct SharedInProcessZoneState {
    next_zone_object_id: u32,
    zone_manager: ZoneManager,
    zone_sessions: BTreeMap<ZonePresenceKey, SessionId>,
    zone_session_keys: BTreeMap<SessionId, ZonePresenceKey>,
    pending_zone_packets: BTreeMap<ZonePresenceKey, Vec<ServerPacket>>,
    pending_zone_transforms: BTreeMap<ZonePresenceKey, (Point, MirDirection)>,
    pending_zone_shout_consumes: BTreeMap<ZonePresenceKey, (bool, bool)>,
    pending_zone_ground_drop_claims: BTreeMap<ZonePresenceKey, Vec<GroundDropSnapshot>>,
    pending_zone_monster_kill_awards: BTreeMap<ZonePresenceKey, Vec<ZoneMonsterKillAward>>,
    pending_zone_player_damages: BTreeMap<ZonePresenceKey, Vec<i32>>,
    pending_zone_player_heals: BTreeMap<ZonePresenceKey, Vec<i32>>,
    players: BTreeMap<ZonePresenceKey, ZonePlayerPresence>,
    maps: BTreeMap<String, ZoneMapSnapshotLayer>,
    trade_offers: BTreeMap<ZonePresenceKey, SharedTradeOffer>,
    pending_trade_deliveries: BTreeMap<ZonePresenceKey, Vec<SharedTradeOffer>>,
    pending_trade_rollbacks: BTreeMap<ZonePresenceKey, Vec<SharedTradeOffer>>,
    pending_rental_invites: BTreeMap<ZonePresenceKey, Vec<SharedItemRentalInvite>>,
    pending_rental_cancels: BTreeMap<ZonePresenceKey, usize>,
    rental_item_offers: BTreeMap<ZonePresenceKey, SharedItemRentalItemOffer>,
    rental_fee_offers: BTreeMap<ZonePresenceKey, SharedItemRentalFeeOffer>,
    pending_rental_deliveries: BTreeMap<ZonePresenceKey, Vec<SharedItemRentalDelivery>>,
    npc_saved_values: BTreeMap<SharedNpcSavedValueKey, SharedNpcSavedValue>,
    npc_random_seed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
enum SharedDropPickupResult {
    Picked(GroundDropSnapshot),
    OwnerBlocked,
    Missing,
}

#[derive(Debug, Clone)]
enum ZoneNativePlayerAttackKind {
    Melee {
        spell: u8,
        attack_type: u8,
    },
    Range {
        target: Point,
        spell: Spell,
        attack_type: u8,
    },
    Magic {
        target: Point,
        spell: Spell,
        cast: bool,
        mp_cost: i32,
        cooldown_ms: u64,
    },
}

#[derive(Debug, Clone)]
struct ZoneNativePlayerAttack {
    object_id: u32,
    direction: MirDirection,
    level: u8,
    damage: i32,
    monster: Option<ZoneMonsterSpawn>,
    kind: ZoneNativePlayerAttackKind,
}

impl SharedInProcessZoneState {
    fn new() -> Self {
        Self {
            next_zone_object_id: 50_000,
            zone_manager: ZoneManager::new(),
            zone_sessions: BTreeMap::new(),
            zone_session_keys: BTreeMap::new(),
            pending_zone_packets: BTreeMap::new(),
            pending_zone_transforms: BTreeMap::new(),
            pending_zone_shout_consumes: BTreeMap::new(),
            pending_zone_ground_drop_claims: BTreeMap::new(),
            pending_zone_monster_kill_awards: BTreeMap::new(),
            pending_zone_player_damages: BTreeMap::new(),
            pending_zone_player_heals: BTreeMap::new(),
            players: BTreeMap::new(),
            maps: BTreeMap::new(),
            trade_offers: BTreeMap::new(),
            pending_trade_deliveries: BTreeMap::new(),
            pending_trade_rollbacks: BTreeMap::new(),
            pending_rental_invites: BTreeMap::new(),
            pending_rental_cancels: BTreeMap::new(),
            rental_item_offers: BTreeMap::new(),
            rental_fee_offers: BTreeMap::new(),
            pending_rental_deliveries: BTreeMap::new(),
            npc_saved_values: BTreeMap::new(),
            npc_random_seed: None,
        }
    }

    fn upsert_player(
        &mut self,
        key: ZonePresenceKey,
        character_name: &str,
        map_file_name: String,
        self_entity: WorldEntitySnapshot,
        free_bag_slots: u16,
    ) -> u32 {
        let existing_presence = self.players.get(&key).cloned();
        let zone_object_id = self
            .players
            .get(&key)
            .map(|presence| presence.zone_object_id)
            .unwrap_or_else(|| {
                let id = self.next_zone_object_id;
                self.next_zone_object_id = self.next_zone_object_id.saturating_add(1);
                id
            });
        let mut entity = self_entity;
        entity.object_id = zone_object_id;
        entity.kind = WorldEntityKind::Player;
        entity.name = character_name.to_string();
        entity.hp = None;
        entity.max_hp = None;
        entity.disposition = WorldEntityDisposition::Friendly;
        if let Some(existing) = existing_presence.as_ref() {
            if existing.map_file_name == map_file_name {
                entity.x = existing.entity.x;
                entity.y = existing.entity.y;
                entity.direction = existing.entity.direction;
            }
        }
        self.players.insert(
            key,
            ZonePlayerPresence {
                zone_object_id,
                map_file_name,
                entity,
                free_bag_slots,
            },
        );
        zone_object_id
    }

    fn remove_player(&mut self, key: &ZonePresenceKey) -> Vec<ZoneOutbound> {
        if let Some(presence) = self.players.get(key).cloned() {
            self.remove_owned_shared_entities(
                &presence.entity.name,
                &presence.map_file_name,
                Some(key),
            );
        }
        self.players.remove(key);
        let Some(session_id) = self.zone_sessions.get(key).cloned() else {
            return Vec::new();
        };
        self.zone_manager.handle(ZoneCommand::Leave { session_id })
    }

    fn forget_zone_session(&mut self, key: &ZonePresenceKey) {
        if let Some(session_id) = self.zone_sessions.remove(key) {
            self.zone_session_keys.remove(&session_id);
        }
        self.pending_zone_packets.remove(key);
        self.pending_zone_transforms.remove(key);
        self.pending_zone_shout_consumes.remove(key);
        self.pending_zone_ground_drop_claims.remove(key);
        self.pending_zone_monster_kill_awards.remove(key);
        self.pending_zone_player_damages.remove(key);
        self.pending_zone_player_heals.remove(key);
    }

    fn zone_session_id_for_key(key: &ZonePresenceKey) -> SessionId {
        SessionId::new(format!("{}:{}", key.account_id, key.character_index))
    }

    fn queue_zone_packets(&mut self, key: ZonePresenceKey, packets: Vec<ServerPacket>) {
        if packets.is_empty() {
            return;
        }
        let pending = self.pending_zone_packets.entry(key).or_default();
        for packet in packets {
            if let Some(object_id) = coalesced_zone_movement_object_id(&packet) {
                pending
                    .retain(|queued| coalesced_zone_movement_object_id(queued) != Some(object_id));
            }
            pending.push(packet);
        }
    }

    fn queue_zone_packets_for_player_names(
        &mut self,
        names: &[String],
        current_key: &ZonePresenceKey,
        packets: Vec<ServerPacket>,
    ) -> Vec<ServerPacket> {
        if packets.is_empty() || names.is_empty() {
            return Vec::new();
        }
        let target_keys = names
            .iter()
            .filter_map(|name| self.player_key_by_name(name))
            .collect::<BTreeSet<_>>();
        let mut current_packets = Vec::new();
        for key in target_keys {
            if &key == current_key {
                current_packets.extend(packets.clone());
            } else {
                self.queue_zone_packets(key, packets.clone());
            }
        }
        current_packets
    }

    fn take_pending_zone_packets(&mut self, key: &ZonePresenceKey) -> Vec<ServerPacket> {
        self.pending_zone_packets.remove(key).unwrap_or_default()
    }

    fn take_pending_zone_transform(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Option<(Point, MirDirection)> {
        self.pending_zone_transforms.remove(key)
    }

    fn update_player_transform(
        &mut self,
        key: &ZonePresenceKey,
        position: Point,
        direction: MirDirection,
    ) {
        if let Some(presence) = self.players.get_mut(key) {
            presence.entity.x = position.x;
            presence.entity.y = position.y;
            presence.entity.direction = direction;
        }
    }

    fn queue_zone_shout_consume(
        &mut self,
        key: ZonePresenceKey,
        map_shout: bool,
        server_shout: bool,
    ) {
        if !map_shout && !server_shout {
            return;
        }
        self.pending_zone_shout_consumes
            .entry(key)
            .and_modify(|pending| {
                pending.0 |= map_shout;
                pending.1 |= server_shout;
            })
            .or_insert((map_shout, server_shout));
    }

    fn take_pending_zone_shout_consume(&mut self, key: &ZonePresenceKey) -> Option<(bool, bool)> {
        self.pending_zone_shout_consumes.remove(key)
    }

    fn queue_zone_ground_drop_claim(&mut self, key: ZonePresenceKey, drop: GroundDropSnapshot) {
        self.pending_zone_ground_drop_claims
            .entry(key)
            .or_default()
            .push(drop);
    }

    fn take_pending_zone_ground_drop_claims(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<GroundDropSnapshot> {
        self.pending_zone_ground_drop_claims
            .remove(key)
            .unwrap_or_default()
    }

    fn queue_zone_monster_kill_award(&mut self, key: ZonePresenceKey, award: ZoneMonsterKillAward) {
        self.pending_zone_monster_kill_awards
            .entry(key)
            .or_default()
            .push(award);
    }

    fn take_pending_zone_monster_kill_awards(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<ZoneMonsterKillAward> {
        self.pending_zone_monster_kill_awards
            .remove(key)
            .unwrap_or_default()
    }

    fn queue_zone_player_damage(&mut self, key: ZonePresenceKey, damage: i32) {
        if damage <= 0 {
            return;
        }
        self.pending_zone_player_damages
            .entry(key)
            .or_default()
            .push(damage);
    }

    fn take_pending_zone_player_damages(&mut self, key: &ZonePresenceKey) -> Vec<i32> {
        self.pending_zone_player_damages
            .remove(key)
            .unwrap_or_default()
    }

    fn queue_zone_player_heal(&mut self, key: ZonePresenceKey, amount: i32) {
        if amount <= 0 {
            return;
        }
        self.pending_zone_player_heals
            .entry(key)
            .or_default()
            .push(amount);
    }

    fn take_pending_zone_player_heals(&mut self, key: &ZonePresenceKey) -> Vec<i32> {
        self.pending_zone_player_heals
            .remove(key)
            .unwrap_or_default()
    }

    fn dispatch_zone_outbounds(
        &mut self,
        outbounds: Vec<ZoneOutbound>,
        current_key: Option<&ZonePresenceKey>,
    ) -> (
        Vec<ServerPacket>,
        Option<(Point, MirDirection)>,
        Option<(bool, bool)>,
        Vec<GroundDropSnapshot>,
        Vec<ZoneMonsterKillAward>,
        Vec<i32>,
        Vec<i32>,
    ) {
        let mut current_packets = Vec::new();
        let mut current_transform = None;
        let mut current_shout_consume = None;
        let mut current_ground_drop_claims = Vec::new();
        let mut current_monster_kill_awards = Vec::new();
        let mut current_player_damages = Vec::new();
        let mut current_player_heals = Vec::new();
        for outbound in outbounds {
            match outbound {
                ZoneOutbound::ToSession {
                    session_id,
                    packets,
                } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    self.apply_zone_packets_to_map_layer(&key, &packets);
                    if current_key == Some(&key) {
                        current_packets.extend(packets);
                    } else {
                        self.queue_zone_packets(key, packets);
                    }
                }
                ZoneOutbound::ToMany {
                    session_ids,
                    packets,
                } => {
                    for session_id in session_ids {
                        let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                            continue;
                        };
                        self.apply_zone_packets_to_map_layer(&key, &packets);
                        if current_key == Some(&key) {
                            current_packets.extend(packets.clone());
                        } else {
                            self.queue_zone_packets(key, packets.clone());
                        }
                    }
                }
                ZoneOutbound::ToAll { packets } => {
                    for key in self.zone_session_keys.values().cloned().collect::<Vec<_>>() {
                        self.apply_zone_packets_to_map_layer(&key, &packets);
                        if current_key == Some(&key) {
                            current_packets.extend(packets.clone());
                        } else {
                            self.queue_zone_packets(key, packets.clone());
                        }
                    }
                }
                ZoneOutbound::SaveTransform {
                    session_id,
                    position,
                    direction,
                } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    self.update_player_transform(&key, position.clone(), direction);
                    if current_key == Some(&key) {
                        current_transform = Some((position, direction));
                    } else {
                        self.pending_zone_transforms
                            .insert(key, (position, direction));
                    }
                }
                ZoneOutbound::ConsumeShoutPermission {
                    session_id,
                    map_shout,
                    server_shout,
                } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if current_key == Some(&key) {
                        current_shout_consume = Some((map_shout, server_shout));
                    } else {
                        self.queue_zone_shout_consume(key, map_shout, server_shout);
                    }
                }
                ZoneOutbound::GroundDropClaimed { session_id, drop } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    self.remove_shared_drop_for_key(&key, drop.object_id);
                    if current_key == Some(&key) {
                        current_ground_drop_claims.push(drop);
                    } else {
                        self.queue_zone_ground_drop_claim(key, drop);
                    }
                }
                ZoneOutbound::MonsterKillAward { session_id, award } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if current_key == Some(&key) {
                        current_monster_kill_awards.push(award);
                    } else {
                        self.queue_zone_monster_kill_award(key, award);
                    }
                }
                ZoneOutbound::PlayerDamaged { session_id, damage } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if current_key == Some(&key) {
                        current_player_damages.push(damage);
                    } else {
                        self.queue_zone_player_damage(key, damage);
                    }
                }
                ZoneOutbound::PlayerHealed { session_id, amount } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if current_key == Some(&key) {
                        current_player_heals.push(amount);
                    } else {
                        self.queue_zone_player_heal(key, amount);
                    }
                }
            }
        }
        (
            current_packets,
            current_transform,
            current_shout_consume,
            current_ground_drop_claims,
            current_monster_kill_awards,
            current_player_damages,
            current_player_heals,
        )
    }

    fn remote_player_entities(
        &self,
        map_file_name: Option<&str>,
        self_key: Option<&ZonePresenceKey>,
    ) -> Vec<WorldEntitySnapshot> {
        let Some(map_file_name) = map_file_name else {
            return Vec::new();
        };
        self.players
            .iter()
            .filter(|(key, presence)| {
                Some(*key) != self_key && presence.map_file_name == map_file_name
            })
            .map(|(_, presence)| presence.entity.clone())
            .collect()
    }

    fn sync_map_layer(
        &mut self,
        map_file_name: String,
        entities: Vec<WorldEntitySnapshot>,
        _previous_entity_ids: BTreeSet<u32>,
        ground_drops: Vec<GroundDropSnapshot>,
        _previous_drop_ids: BTreeSet<u32>,
    ) {
        let now_ms = shared_gateway_now_ms();
        let map = self.maps.entry(map_file_name).or_default();
        for mut entity in entities {
            if !map.removed_entity_ids.contains(&entity.object_id) {
                if let Some(dead) = map.dead_entity_ids.get(&entity.object_id) {
                    entity.dead = true;
                    entity.hp = Some(0);
                    if let Some(location) = dead.location.as_ref() {
                        entity.x = location.x;
                        entity.y = location.y;
                    }
                    if let Some(direction) = dead.direction {
                        entity.direction = direction;
                    }
                } else if map.revived_entity_ids.contains(&entity.object_id)
                    && (entity.dead || entity.hp.is_some_and(|hp| hp <= 0))
                {
                    entity.dead = false;
                    if let Some(max_hp) = entity.max_hp {
                        entity.hp = Some(max_hp);
                    }
                }
                let entity = merge_shared_entity_state(map.entities.get(&entity.object_id), entity);
                map.entities.insert(entity.object_id, entity);
            }
        }

        for drop in ground_drops {
            if !map.removed_drop_ids.contains(&drop.object_id) {
                if !map.ground_drops.contains_key(&drop.object_id)
                    && drop_matches_committed_death(map, &drop)
                {
                    map.removed_drop_ids.insert(drop.object_id);
                    map.drop_ownership_expires_at_ms.remove(&drop.object_id);
                    map.drop_expires_at_ms.remove(&drop.object_id);
                    continue;
                }
                Self::sync_drop_ownership_deadline(map, &drop, now_ms);
                map.ground_drops.insert(drop.object_id, drop);
            }
        }
    }

    fn sync_drop_ownership_deadline(
        map: &mut ZoneMapSnapshotLayer,
        drop: &GroundDropSnapshot,
        now_ms: u64,
    ) {
        map.drop_expires_at_ms
            .entry(drop.object_id)
            .or_insert_with(|| {
                now_ms
                    .saturating_add(SHARED_DROP_EXPIRE_TICKS.saturating_mul(SHARED_CRYSTAL_TICK_MS))
            });
        if let Some(remaining_ticks) = drop
            .ownership_remaining_ticks
            .filter(|remaining_ticks| *remaining_ticks > 0)
        {
            map.drop_ownership_expires_at_ms.insert(
                drop.object_id,
                now_ms.saturating_add(remaining_ticks.saturating_mul(SHARED_CRYSTAL_TICK_MS)),
            );
        } else {
            map.drop_ownership_expires_at_ms.remove(&drop.object_id);
        }
    }

    #[cfg(test)]
    fn expire_drop_ownership_if_due(&mut self, map_file_name: &str, object_id: u32, now_ms: u64) {
        let Some(map) = self.maps.get_mut(map_file_name) else {
            return;
        };
        if map
            .drop_ownership_expires_at_ms
            .get(&object_id)
            .is_some_and(|expires_at_ms| now_ms >= *expires_at_ms)
        {
            map.drop_ownership_expires_at_ms.remove(&object_id);
            if let Some(drop) = map.ground_drops.get_mut(&object_id) {
                drop.ownership_remaining_ticks = None;
            }
        }
    }

    #[cfg(test)]
    fn expire_drop_ownerships(&mut self, map_file_name: &str, now_ms: u64) {
        let object_ids = self
            .maps
            .get(map_file_name)
            .map(|map| {
                map.drop_ownership_expires_at_ms
                    .iter()
                    .filter_map(|(object_id, expires_at_ms)| {
                        (now_ms >= *expires_at_ms).then_some(*object_id)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for object_id in object_ids {
            self.expire_drop_ownership_if_due(map_file_name, object_id, now_ms);
        }
    }

    fn expire_shared_drops(
        &mut self,
        map_file_name: &str,
        current_key: Option<&ZonePresenceKey>,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let object_ids = self
            .maps
            .get(map_file_name)
            .map(|map| {
                map.drop_expires_at_ms
                    .iter()
                    .filter_map(|(object_id, expires_at_ms)| {
                        (now_ms >= *expires_at_ms).then_some(*object_id)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if object_ids.is_empty() {
            return Vec::new();
        }
        let Some(map) = self.maps.get_mut(map_file_name) else {
            return Vec::new();
        };
        for object_id in &object_ids {
            map.ground_drops.remove(object_id);
            map.removed_drop_ids.insert(*object_id);
            map.drop_ownership_expires_at_ms.remove(object_id);
            map.drop_expires_at_ms.remove(object_id);
        }
        let packets = object_ids
            .into_iter()
            .map(|object_id| ServerPacket::ObjectRemove { object_id })
            .collect::<Vec<_>>();
        let current_receives = current_key.is_some_and(|key| {
            self.players
                .get(key)
                .is_some_and(|presence| presence.map_file_name == map_file_name)
        });
        let recipients = self
            .players
            .iter()
            .filter(|(key, presence)| {
                Some(*key) != current_key && presence.map_file_name == map_file_name
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in recipients {
            self.queue_zone_packets(key, packets.clone());
        }
        if current_receives {
            packets
        } else {
            Vec::new()
        }
    }

    fn commit_death_drops(
        &mut self,
        map_file_name: &str,
        packets: &[ServerPacket],
        ground_drops: &[GroundDropSnapshot],
    ) -> Vec<GroundDropSnapshot> {
        if packets.is_empty() || ground_drops.is_empty() {
            return Vec::new();
        }
        let Some(map) = self.maps.get_mut(map_file_name) else {
            return Vec::new();
        };
        let now_ms = shared_gateway_now_ms();
        let mut committed = Vec::new();
        let deaths = death_drop_anchors(map, packets);
        for (monster_object_id, monster_name, location) in deaths {
            if map
                .committed_death_drop_anchors
                .contains_key(&monster_object_id)
            {
                continue;
            }
            map.committed_death_drop_anchors.insert(
                monster_object_id,
                SharedDeathDropAnchor {
                    monster_name: monster_name.clone(),
                    location: location.clone(),
                },
            );
            for drop in ground_drops
                .iter()
                .filter(|drop| drop_matches_death_anchor(drop, monster_name.as_deref(), &location))
            {
                if !map.removed_drop_ids.contains(&drop.object_id)
                    && !map.ground_drops.contains_key(&drop.object_id)
                {
                    Self::sync_drop_ownership_deadline(map, drop, now_ms);
                    map.ground_drops.insert(drop.object_id, drop.clone());
                    committed.push(drop.clone());
                }
            }
        }
        committed
    }

    fn apply_shared_entity_packets(&mut self, map_file_name: &str, packets: &[ServerPacket]) {
        let player_names_by_zone_object_id = self
            .players
            .values()
            .map(|presence| (presence.zone_object_id, presence.entity.name.clone()))
            .collect::<BTreeMap<_, _>>();
        let map = self.maps.entry(map_file_name.to_string()).or_default();
        for packet in packets {
            match packet {
                ServerPacket::ObjectHealth { info } => {
                    let mut died = false;
                    if let Some(entity) = map.entities.get_mut(&info.object_id) {
                        if entity.dead {
                            continue;
                        }
                        if info.percent == 0 {
                            entity.hp = Some(0);
                            entity.dead = true;
                            died = true;
                        } else if let Some(max_hp) = entity.max_hp {
                            let percent = i32::from(info.percent).clamp(0, 100);
                            let hp = (max_hp.saturating_mul(percent).saturating_add(99)) / 100;
                            let hp = hp.clamp(0, max_hp);
                            entity.hp = Some(entity.hp.map_or(hp, |current| current.min(hp)));
                        }
                    }
                    if died {
                        map.revived_entity_ids.remove(&info.object_id);
                        let location = map.entities.get(&info.object_id).map(|entity| Point {
                            x: entity.x,
                            y: entity.y,
                        });
                        let direction = map
                            .entities
                            .get(&info.object_id)
                            .map(|entity| entity.direction);
                        map.dead_entity_ids.insert(
                            info.object_id,
                            SharedDeadEntityState {
                                location,
                                direction,
                            },
                        );
                    } else if info.percent == 0 {
                        map.revived_entity_ids.remove(&info.object_id);
                        map.dead_entity_ids.insert(
                            info.object_id,
                            SharedDeadEntityState {
                                location: None,
                                direction: None,
                            },
                        );
                    }
                }
                ServerPacket::ObjectDied { info } => {
                    if let Some(entity) = map.entities.get_mut(&info.object_id) {
                        entity.x = info.location.x;
                        entity.y = info.location.y;
                        entity.direction = info.direction;
                        entity.hp = Some(0);
                        entity.dead = true;
                    }
                    map.revived_entity_ids.remove(&info.object_id);
                    map.dead_entity_ids.insert(
                        info.object_id,
                        SharedDeadEntityState {
                            location: Some(info.location.clone()),
                            direction: Some(info.direction),
                        },
                    );
                }
                ServerPacket::ObjectRevived { info } => {
                    map.removed_entity_ids.remove(&info.object_id);
                    map.dead_entity_ids.remove(&info.object_id);
                    map.revived_entity_ids.insert(info.object_id);
                    map.committed_death_drop_anchors.remove(&info.object_id);
                    map.harvested_entity_ids.remove(&info.object_id);
                    if let Some(entity) = map.entities.get_mut(&info.object_id) {
                        entity.dead = false;
                        if let Some(max_hp) = entity.max_hp {
                            entity.hp = Some(max_hp);
                        }
                    }
                }
                ServerPacket::ObjectRemove { object_id } => {
                    map.entities.remove(object_id);
                    map.removed_entity_ids.insert(*object_id);
                    map.ground_drops.remove(object_id);
                    map.removed_drop_ids.insert(*object_id);
                    map.drop_ownership_expires_at_ms.remove(object_id);
                    map.drop_expires_at_ms.remove(object_id);
                    map.harvested_entity_ids.remove(object_id);
                    map.revived_entity_ids.remove(object_id);
                    map.dead_entity_ids.remove(object_id);
                }
                ServerPacket::ObjectHarvested { movement } => {
                    map.harvested_entity_ids.insert(movement.object_id);
                }
                ServerPacket::ObjectTurn { movement }
                | ServerPacket::ObjectWalk { movement }
                | ServerPacket::ObjectRun { movement }
                | ServerPacket::ObjectBackStep { movement, .. }
                | ServerPacket::ObjectSitDown { movement, .. } => {
                    apply_shared_entity_transform(
                        map,
                        movement.object_id,
                        &movement.position,
                        movement.direction,
                    );
                }
                ServerPacket::ObjectDash {
                    object_id,
                    location,
                    direction,
                }
                | ServerPacket::ObjectDashFail {
                    object_id,
                    location,
                    direction,
                }
                | ServerPacket::ObjectDashAttack {
                    object_id,
                    location,
                    direction,
                    ..
                }
                | ServerPacket::ObjectPushed {
                    object_id,
                    location,
                    direction,
                } => {
                    apply_shared_entity_transform(map, *object_id, location, *direction);
                }
                ServerPacket::IntelligentCreaturePickup { object_id } => {
                    map.ground_drops.remove(object_id);
                    map.removed_drop_ids.insert(*object_id);
                    map.drop_ownership_expires_at_ms.remove(object_id);
                    map.drop_expires_at_ms.remove(object_id);
                }
                ServerPacket::ObjectMonster { info } => {
                    if !map.removed_entity_ids.contains(&info.object_id) {
                        let mut entity = world_entity_from_monster_info(info);
                        entity.owner_name = player_names_by_zone_object_id
                            .get(&info.master_object_id)
                            .cloned();
                        if let Some(dead) = map.dead_entity_ids.get(&info.object_id) {
                            entity.dead = true;
                            entity.hp = Some(0);
                            if let Some(location) = dead.location.as_ref() {
                                entity.x = location.x;
                                entity.y = location.y;
                            }
                            if let Some(direction) = dead.direction {
                                entity.direction = direction;
                            }
                        } else if info.dead {
                            map.dead_entity_ids.insert(
                                info.object_id,
                                SharedDeadEntityState {
                                    location: Some(info.location.clone()),
                                    direction: Some(info.direction),
                                },
                            );
                        } else {
                            map.revived_entity_ids.remove(&info.object_id);
                        }
                        map.entities.insert(info.object_id, entity);
                    }
                }
                ServerPacket::ObjectHero { info, owner_name } => {
                    if !map.removed_entity_ids.contains(&info.object_id) {
                        map.dead_entity_ids.remove(&info.object_id);
                        map.revived_entity_ids.remove(&info.object_id);
                        map.entities.insert(
                            info.object_id,
                            world_entity_from_object_player_info(info, Some(owner_name.clone())),
                        );
                    }
                }
                ServerPacket::ObjectNpc { info } => {
                    if !map.removed_entity_ids.contains(&info.object_id) {
                        map.entities
                            .insert(info.object_id, world_entity_from_npc_info(info));
                    }
                }
                _ => {}
            }
        }
    }

    fn shared_entity_allows_action(&self, map_file_name: &str, object_id: u32) -> bool {
        let Some(map) = self.maps.get(map_file_name) else {
            return true;
        };
        if map.removed_entity_ids.contains(&object_id) {
            return false;
        }
        if map.dead_entity_ids.contains_key(&object_id) {
            return false;
        }
        !map.entities
            .get(&object_id)
            .is_some_and(|entity| entity.dead || entity.hp.is_some_and(|hp| hp <= 0))
    }

    fn shared_entity(&self, map_file_name: &str, object_id: u32) -> Option<WorldEntitySnapshot> {
        let map = self.maps.get(map_file_name)?;
        if map.removed_entity_ids.contains(&object_id) {
            return None;
        }
        map.entities.get(&object_id).cloned()
    }

    fn shared_harvest_allows_action(
        &self,
        map_file_name: &str,
        picker: &WorldEntitySnapshot,
        direction: MirDirection,
    ) -> bool {
        let Some(map) = self.maps.get(map_file_name) else {
            return true;
        };
        let target = point_in_direction(
            &Point {
                x: picker.x,
                y: picker.y,
            },
            direction,
        );
        let mut dead_shared_corpses = map
            .entities
            .values()
            .filter(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.x == target.x
                    && entity.y == target.y
                    && !map.removed_entity_ids.contains(&entity.object_id)
                    && (entity.dead || entity.hp.is_some_and(|hp| hp <= 0))
            })
            .peekable();
        if dead_shared_corpses.peek().is_none() {
            return true;
        }
        dead_shared_corpses.any(|entity| !map.harvested_entity_ids.contains(&entity.object_id))
    }

    fn shared_entities(&self, map_file_name: &str) -> Vec<WorldEntitySnapshot> {
        let Some(map) = self.maps.get(map_file_name) else {
            return Vec::new();
        };
        map.entities
            .values()
            .filter(|entity| !map.removed_entity_ids.contains(&entity.object_id))
            .cloned()
            .collect()
    }

    fn shared_npc_entity(
        &self,
        map_file_name: &str,
        object_id: u32,
    ) -> Option<WorldEntitySnapshot> {
        self.shared_entity(map_file_name, object_id)
            .filter(|entity| entity.kind == WorldEntityKind::Npc)
    }

    fn shared_npc_saved_values(&self) -> Vec<SharedNpcSavedValue> {
        self.npc_saved_values.values().cloned().collect()
    }

    fn merge_shared_npc_saved_values(&mut self, values: Vec<SharedNpcSavedValue>) {
        for value in values {
            self.npc_saved_values
                .insert(SharedNpcSavedValueKey::from_value(&value), value);
        }
    }

    fn shared_npc_random_seed(&self) -> Option<u64> {
        self.npc_random_seed
    }

    fn merge_shared_npc_random_seed(&mut self, seed: u64) {
        self.npc_random_seed = Some(seed);
    }

    fn map_layer(&self, map_file_name: Option<&str>) -> Option<ZoneMapSnapshotLayer> {
        let map_file_name = map_file_name?;
        self.maps.get(map_file_name).cloned()
    }

    fn has_ground_drops_for_map(&self, map_file_name: Option<&str>) -> Option<bool> {
        let map_file_name = map_file_name?;
        self.maps
            .get(map_file_name)
            .map(|map| !map.ground_drops.is_empty())
    }

    #[cfg(test)]
    fn take_pickable_drop(
        &mut self,
        map_file_name: &str,
        object_id: Option<u32>,
        picker: &WorldEntitySnapshot,
        picker_zone_object_id: u32,
        picker_group_members: &[String],
    ) -> SharedDropPickupResult {
        let picker_location = Point {
            x: picker.x,
            y: picker.y,
        };
        self.take_pickable_drop_at(
            map_file_name,
            object_id,
            &picker_location,
            picker_zone_object_id,
            picker_group_members,
        )
    }

    #[cfg(test)]
    fn take_pickable_drop_at(
        &mut self,
        map_file_name: &str,
        object_id: Option<u32>,
        target_location: &Point,
        picker_zone_object_id: u32,
        picker_group_members: &[String],
    ) -> SharedDropPickupResult {
        let object_id = {
            let Some(map) = self.maps.get(map_file_name) else {
                return SharedDropPickupResult::Missing;
            };
            match object_id {
                Some(object_id) => object_id,
                None => map
                    .ground_drops
                    .values()
                    .find(|drop| drop.x == target_location.x && drop.y == target_location.y)
                    .map(|drop| drop.object_id)
                    .unwrap_or_default(),
            }
        };
        if object_id == 0 {
            return SharedDropPickupResult::Missing;
        }
        let now_ms = shared_gateway_now_ms();
        self.expire_drop_ownership_if_due(map_file_name, object_id, now_ms);
        let Some(map) = self.maps.get(map_file_name) else {
            return SharedDropPickupResult::Missing;
        };
        let Some(drop) = map.ground_drops.get(&object_id) else {
            return SharedDropPickupResult::Missing;
        };
        if !self.drop_ownership_allows_pickup(drop, picker_zone_object_id, picker_group_members) {
            return SharedDropPickupResult::OwnerBlocked;
        }
        if drop.x != target_location.x || drop.y != target_location.y {
            return SharedDropPickupResult::Missing;
        }
        let Some(map) = self.maps.get_mut(map_file_name) else {
            return SharedDropPickupResult::Missing;
        };
        let Some(drop) = map.ground_drops.remove(&object_id) else {
            return SharedDropPickupResult::Missing;
        };
        map.removed_drop_ids.insert(object_id);
        map.drop_ownership_expires_at_ms.remove(&object_id);
        map.drop_expires_at_ms.remove(&object_id);
        SharedDropPickupResult::Picked(drop)
    }

    #[cfg(test)]
    fn take_auto_pickable_drop_for_creature(
        &mut self,
        map_file_name: &str,
        picker_location: &Point,
        picker_zone_object_id: u32,
        picker_group_members: &[String],
        creature: &mir2_protocol::ClientIntelligentCreature,
    ) -> SharedDropPickupResult {
        let range = creature.creature_rules.auto_pickup_range;
        if range <= 0 {
            return SharedDropPickupResult::Missing;
        }
        self.expire_drop_ownerships(map_file_name, shared_gateway_now_ms());
        let Some((object_id, target_location)) = self.maps.get(map_file_name).and_then(|map| {
            map.ground_drops
                .values()
                .filter_map(|drop| {
                    let distance = (drop.x - picker_location.x)
                        .abs()
                        .max((drop.y - picker_location.y).abs());
                    if distance > range
                        || !intelligent_creature_allows_ground_drop(creature, drop)
                        || !self.drop_ownership_allows_pickup(
                            drop,
                            picker_zone_object_id,
                            picker_group_members,
                        )
                    {
                        return None;
                    }
                    Some((
                        distance,
                        drop.object_id,
                        Point {
                            x: drop.x,
                            y: drop.y,
                        },
                    ))
                })
                .min_by_key(|(distance, object_id, _)| (*distance, *object_id))
                .map(|(_, object_id, location)| (object_id, location))
        }) else {
            return SharedDropPickupResult::Missing;
        };
        self.take_pickable_drop_at(
            map_file_name,
            Some(object_id),
            &target_location,
            picker_zone_object_id,
            picker_group_members,
        )
    }

    #[cfg(test)]
    fn drop_ownership_allows_pickup(
        &self,
        drop: &GroundDropSnapshot,
        picker_zone_object_id: u32,
        picker_group_members: &[String],
    ) -> bool {
        if !drop
            .ownership_remaining_ticks
            .is_some_and(|remaining_ticks| remaining_ticks > 0)
        {
            return true;
        }
        let Some(owner_object_id) = drop.owner_object_id else {
            return true;
        };
        if owner_object_id == picker_zone_object_id {
            return true;
        }
        self.player_name_by_zone_object_id(owner_object_id)
            .is_some_and(|owner_name| {
                picker_group_members
                    .iter()
                    .any(|member| member.eq_ignore_ascii_case(&owner_name))
            })
    }

    fn restore_drop(&mut self, map_file_name: &str, drop: GroundDropSnapshot) {
        let map = self.maps.entry(map_file_name.to_string()).or_default();
        map.removed_drop_ids.remove(&drop.object_id);
        Self::sync_drop_ownership_deadline(map, &drop, shared_gateway_now_ms());
        map.ground_drops.insert(drop.object_id, drop);
    }

    fn restore_drop_for_key(&mut self, key: &ZonePresenceKey, drop: GroundDropSnapshot) {
        let Some(map_file_name) = self
            .players
            .get(key)
            .map(|presence| presence.map_file_name.clone())
        else {
            return;
        };
        self.restore_drop(&map_file_name, drop);
    }

    fn remove_shared_drop_for_key(&mut self, key: &ZonePresenceKey, object_id: u32) {
        let Some(map_file_name) = self
            .players
            .get(key)
            .map(|presence| presence.map_file_name.clone())
        else {
            return;
        };
        let Some(map) = self.maps.get_mut(&map_file_name) else {
            return;
        };
        if map.ground_drops.remove(&object_id).is_some() {
            map.removed_drop_ids.insert(object_id);
        }
        map.drop_ownership_expires_at_ms.remove(&object_id);
        map.drop_expires_at_ms.remove(&object_id);
    }

    fn apply_zone_packets_to_map_layer(&mut self, key: &ZonePresenceKey, packets: &[ServerPacket]) {
        for packet in packets {
            match packet {
                ServerPacket::ObjectRemove { object_id }
                | ServerPacket::IntelligentCreaturePickup { object_id } => {
                    self.remove_shared_drop_for_key(key, *object_id);
                }
                _ => {}
            }
        }
    }

    fn player_key_by_name(&self, name: &str) -> Option<ZonePresenceKey> {
        self.players
            .iter()
            .find(|(_, presence)| presence.entity.name.eq_ignore_ascii_case(name))
            .map(|(key, _)| key.clone())
    }

    #[cfg(test)]
    fn player_name_by_zone_object_id(&self, zone_object_id: u32) -> Option<String> {
        self.players
            .values()
            .find(|presence| presence.zone_object_id == zone_object_id)
            .map(|presence| presence.entity.name.clone())
    }

    fn remove_owned_shared_entities(
        &mut self,
        owner_name: &str,
        map_file_name: &str,
        excluded_key: Option<&ZonePresenceKey>,
    ) {
        let Some(map) = self.maps.get_mut(map_file_name) else {
            return;
        };
        let object_ids = map
            .entities
            .values()
            .filter(|entity| {
                entity
                    .owner_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(owner_name))
            })
            .map(|entity| entity.object_id)
            .collect::<Vec<_>>();
        if object_ids.is_empty() {
            return;
        }
        for object_id in &object_ids {
            map.entities.remove(object_id);
            map.removed_entity_ids.insert(*object_id);
            map.dead_entity_ids.remove(object_id);
            map.revived_entity_ids.remove(object_id);
            map.harvested_entity_ids.remove(object_id);
            map.committed_death_drop_anchors.remove(object_id);
        }
        let packets = object_ids
            .into_iter()
            .map(|object_id| ServerPacket::ObjectRemove { object_id })
            .collect::<Vec<_>>();
        let recipients = self
            .players
            .iter()
            .filter(|(key, presence)| {
                Some(*key) != excluded_key && presence.map_file_name == map_file_name
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in recipients {
            self.queue_zone_packets(key, packets.clone());
        }
    }

    fn take_pending_trade_deliveries(&mut self, key: &ZonePresenceKey) -> Vec<SharedTradeOffer> {
        self.pending_trade_deliveries
            .remove(key)
            .unwrap_or_default()
    }

    fn take_pending_trade_rollbacks(&mut self, key: &ZonePresenceKey) -> Vec<SharedTradeOffer> {
        self.pending_trade_rollbacks.remove(key).unwrap_or_default()
    }

    fn queue_rental_invite(&mut self, key: ZonePresenceKey, partner_name: String, renting: bool) {
        self.pending_rental_invites
            .entry(key)
            .or_default()
            .push(SharedItemRentalInvite {
                partner_name,
                renting,
            });
    }

    fn take_pending_rental_invites(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<SharedItemRentalInvite> {
        self.pending_rental_invites.remove(key).unwrap_or_default()
    }

    fn queue_rental_cancel(&mut self, key: ZonePresenceKey) {
        *self.pending_rental_cancels.entry(key).or_default() += 1;
    }

    fn take_pending_rental_cancel_count(&mut self, key: &ZonePresenceKey) -> usize {
        self.pending_rental_cancels.remove(key).unwrap_or_default()
    }

    fn take_pending_rental_deliveries(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<SharedItemRentalDelivery> {
        self.pending_rental_deliveries
            .remove(key)
            .unwrap_or_default()
    }

    fn rental_fee_offer_matching_item(
        &self,
        item_offer: &SharedItemRentalItemOffer,
    ) -> Option<(ZonePresenceKey, SharedItemRentalFeeOffer)> {
        self.rental_fee_offers
            .iter()
            .find(|(_, fee_offer)| {
                fee_offer
                    .character_name
                    .eq_ignore_ascii_case(&item_offer.partner_name)
                    && fee_offer
                        .partner_name
                        .eq_ignore_ascii_case(&item_offer.character_name)
            })
            .map(|(key, offer)| (key.clone(), offer.clone()))
    }

    fn rental_item_offer_matching_fee(
        &self,
        fee_offer: &SharedItemRentalFeeOffer,
    ) -> Option<(ZonePresenceKey, SharedItemRentalItemOffer)> {
        self.rental_item_offers
            .iter()
            .find(|(_, item_offer)| {
                item_offer
                    .character_name
                    .eq_ignore_ascii_case(&fee_offer.partner_name)
                    && item_offer
                        .partner_name
                        .eq_ignore_ascii_case(&fee_offer.character_name)
            })
            .map(|(key, offer)| (key.clone(), offer.clone()))
    }

    fn cancel_rental_offers_for_presence(
        &mut self,
        key: &ZonePresenceKey,
        character_name: &str,
    ) -> Vec<ZonePresenceKey> {
        let mut cancel_keys = Vec::new();
        if let Some(item_offer) = self.rental_item_offers.remove(key) {
            if let Some((fee_key, _)) = self.rental_fee_offer_matching_item(&item_offer) {
                self.rental_fee_offers.remove(&fee_key);
                cancel_keys.push(fee_key);
            }
        }
        if let Some(fee_offer) = self.rental_fee_offers.remove(key) {
            if let Some((item_key, _)) = self.rental_item_offer_matching_fee(&fee_offer) {
                self.rental_item_offers.remove(&item_key);
                cancel_keys.push(item_key);
            }
        }

        let item_keys = self
            .rental_item_offers
            .iter()
            .filter(|(_, offer)| {
                offer.partner_name.eq_ignore_ascii_case(character_name)
                    || offer.character_name.eq_ignore_ascii_case(character_name)
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for item_key in item_keys {
            self.rental_item_offers.remove(&item_key);
            cancel_keys.push(item_key);
        }
        let fee_keys = self
            .rental_fee_offers
            .iter()
            .filter(|(_, offer)| {
                offer.partner_name.eq_ignore_ascii_case(character_name)
                    || offer.character_name.eq_ignore_ascii_case(character_name)
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for fee_key in fee_keys {
            self.rental_fee_offers.remove(&fee_key);
            cancel_keys.push(fee_key);
        }
        cancel_keys.sort();
        cancel_keys.dedup();
        cancel_keys
    }

    fn cancel_trade_offers_for_presence(
        &mut self,
        key: &ZonePresenceKey,
        character_name: &str,
    ) -> Option<SharedTradeOffer> {
        let own_offer = self.trade_offers.remove(key);
        let owner_keys = self
            .trade_offers
            .iter()
            .filter(|(_, offer)| offer.partner_name.eq_ignore_ascii_case(character_name))
            .map(|(owner_key, _)| owner_key.clone())
            .collect::<Vec<_>>();
        for owner_key in owner_keys {
            if let Some(offer) = self.trade_offers.remove(&owner_key) {
                self.pending_trade_rollbacks
                    .entry(owner_key)
                    .or_default()
                    .push(offer);
            }
        }
        own_offer
    }
}

#[derive(Debug, Clone)]
pub struct SharedInProcessZoneRuntimeFactory {
    states: Arc<Mutex<BTreeMap<ZoneId, Arc<Mutex<SharedInProcessZoneState>>>>>,
    account_inventory_service: SharedAccountInventoryServiceHandle,
    npc_world_service: SharedNpcWorldServiceHandle,
}

impl SharedInProcessZoneRuntimeFactory {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service: Arc::new(InProcessAccountInventoryService::new()),
            npc_world_service: Arc::new(InProcessNpcWorldService),
        }
    }

    pub fn with_account_inventory_service(
        account_inventory_service: SharedAccountInventoryServiceHandle,
    ) -> Self {
        Self {
            states: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service,
            npc_world_service: Arc::new(InProcessNpcWorldService),
        }
    }

    pub fn with_world_services(
        account_inventory_service: SharedAccountInventoryServiceHandle,
        npc_world_service: SharedNpcWorldServiceHandle,
    ) -> Self {
        Self {
            states: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service,
            npc_world_service,
        }
    }

    fn state_for_zone(&self, zone_id: &ZoneId) -> Arc<Mutex<SharedInProcessZoneState>> {
        self.states
            .lock()
            .expect("shared zone factory mutex should not be poisoned")
            .entry(zone_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(SharedInProcessZoneState::new())))
            .clone()
    }
}

impl Default for SharedInProcessZoneRuntimeFactory {
    fn default() -> Self {
        Self::new()
    }
}

fn merge_shared_entity_state(
    existing: Option<&WorldEntitySnapshot>,
    mut incoming: WorldEntitySnapshot,
) -> WorldEntitySnapshot {
    let Some(existing) = existing else {
        return incoming;
    };
    if incoming.owner_name.is_none() {
        incoming.owner_name = existing.owner_name.clone();
    }
    if existing.dead {
        incoming.dead = true;
        incoming.hp = Some(0);
        incoming.x = existing.x;
        incoming.y = existing.y;
        incoming.direction = existing.direction;
        return incoming;
    }
    if let (Some(existing_hp), Some(incoming_hp)) = (existing.hp, incoming.hp) {
        if existing_hp < incoming_hp {
            incoming.hp = Some(existing_hp);
        }
    }
    if incoming.sprite.is_none() {
        incoming.sprite = existing.sprite.clone();
    }
    incoming
}

fn simple_world_entity_sprite_snapshot(
    library_root: &str,
    image: u16,
    padding: usize,
) -> WorldEntitySpriteSnapshot {
    WorldEntitySpriteSnapshot {
        body_library: format!("{}/{:0width$}", library_root, image, width = padding),
        hair_library: None,
        weapon_library: None,
        weapon_library_secondary: None,
        frame_base_offset: 0,
        weapon_frame_offset: None,
        alt_body_library: None,
        alt_hair_library: None,
        alt_weapon_library: None,
        alt_weapon_library_secondary: None,
        alt_frame_base_offset: None,
        alt_weapon_frame_offset: None,
        frame_count: 4,
        direction_stride: 4,
        mount_library: None,
        mount_frame_offset: None,
    }
}

fn sprite_image_from_shared_entity(entity: &WorldEntitySnapshot, library_root: &str) -> u16 {
    let Some(sprite) = entity.sprite.as_ref() else {
        return 0;
    };
    let Some(image) = sprite
        .body_library
        .strip_prefix(&format!("{library_root}/"))
    else {
        return 0;
    };
    image.parse::<u16>().unwrap_or_default()
}

fn world_entity_from_monster_info(info: &MonsterInfo) -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id: info.object_id,
        kind: WorldEntityKind::Monster,
        name: info.name.clone(),
        owner_name: None,
        x: info.location.x,
        y: info.location.y,
        direction: info.direction,
        class: None,
        gender: None,
        level: None,
        hp: None,
        max_hp: None,
        name_colour_argb: info.name_colour_argb,
        dead: info.dead,
        disposition: world_entity_disposition_for_monster_ai(info.ai),
        sprite: Some(simple_world_entity_sprite_snapshot(
            "Monster", info.image, 3,
        )),
        quest_ids: Vec::new(),
    }
}

fn world_entity_disposition_for_monster_ai(ai: u8) -> WorldEntityDisposition {
    match ai {
        1 | 2 | 3 | 6 | 34 | 56 | 57 | 58 | 113 => WorldEntityDisposition::Neutral,
        _ => WorldEntityDisposition::Hostile,
    }
}

fn fallback_monster_ai_for_shared_entity(entity: &WorldEntitySnapshot) -> u8 {
    match entity.disposition {
        WorldEntityDisposition::Friendly | WorldEntityDisposition::Neutral => 1,
        WorldEntityDisposition::Hostile => 0,
    }
}

fn zone_monster_spawn_from_shared_entity(
    entity: &WorldEntitySnapshot,
    current_tick: u64,
) -> Option<ZoneMonsterSpawn> {
    if entity.kind != WorldEntityKind::Monster || entity.dead {
        return None;
    }
    let template = crystal_monster_by_name(&entity.name);
    let template_ref = template.as_ref();
    let max_hp = entity
        .max_hp
        .or(entity.hp)
        .or_else(|| template_ref.map(|monster| monster.hp))
        .unwrap_or(1)
        .max(1);
    let hp = entity.hp.unwrap_or(max_hp).clamp(0, max_hp);
    Some(ZoneMonsterSpawn {
        object_id: entity.object_id,
        name: entity.name.clone(),
        name_colour_argb: entity.name_colour_argb,
        image: entity
            .sprite
            .as_ref()
            .and_then(|sprite| {
                sprite
                    .body_library
                    .strip_prefix("Monster/")
                    .and_then(|value| value.parse::<u16>().ok())
            })
            .or_else(|| template_ref.map(|monster| monster.image))
            .unwrap_or_default(),
        ai: template_ref
            .map(|monster| monster.ai)
            .unwrap_or_else(|| fallback_monster_ai_for_shared_entity(entity)),
        level: entity
            .level
            .or_else(|| template_ref.map(|monster| monster.level))
            .unwrap_or(1),
        max_hp,
        hp,
        experience: template_ref.map(|monster| monster.experience).unwrap_or(0),
        defense: template_ref
            .map(|monster| ZoneMonsterDefense::from_crystal_template(monster))
            .unwrap_or_default(),
        position: Point {
            x: entity.x,
            y: entity.y,
        },
        direction: entity.direction,
        drops: zone_ground_drop_snapshots_for_monster_at_tick(
            entity.object_id,
            &entity.name,
            current_tick,
        ),
    })
}

fn world_entity_from_object_player_info(
    info: &ObjectPlayerInfo,
    owner_name: Option<String>,
) -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id: info.object_id,
        kind: WorldEntityKind::Player,
        name: info.name.clone(),
        owner_name,
        x: info.location.x,
        y: info.location.y,
        direction: info.direction,
        class: Some(info.class),
        gender: Some(info.gender),
        level: Some(info.level),
        hp: None,
        max_hp: None,
        name_colour_argb: info.name_colour_argb,
        dead: info.dead,
        disposition: WorldEntityDisposition::Friendly,
        sprite: None,
        quest_ids: Vec::new(),
    }
}

fn world_entity_from_npc_info(info: &NpcInfo) -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id: info.object_id,
        kind: WorldEntityKind::Npc,
        name: info.name.clone(),
        owner_name: None,
        x: info.location.x,
        y: info.location.y,
        direction: info.direction,
        class: None,
        gender: None,
        level: None,
        hp: None,
        max_hp: None,
        name_colour_argb: info.name_colour_argb,
        dead: false,
        disposition: WorldEntityDisposition::Neutral,
        sprite: Some(simple_world_entity_sprite_snapshot("NPC", info.image, 2)),
        quest_ids: info.quest_ids.clone(),
    }
}

fn shared_entity_spawn_packet(entity: &WorldEntitySnapshot) -> Option<ServerPacket> {
    let location = Point {
        x: entity.x,
        y: entity.y,
    };
    match entity.kind {
        WorldEntityKind::Monster => Some(ServerPacket::ObjectMonster {
            info: MonsterInfo {
                object_id: entity.object_id,
                name: entity.name.clone(),
                name_colour_argb: entity.name_colour_argb,
                location,
                image: sprite_image_from_shared_entity(entity, "Monster"),
                direction: entity.direction,
                effect: 0,
                ai: 0,
                light: 0,
                dead: entity.dead || entity.hp.is_some_and(|hp| hp <= 0),
                skeleton: false,
                poison: 0,
                hidden: false,
                shock_time: 0,
                binding_shot_center: false,
                extra: false,
                extra_byte: 0,
                master_object_id: 0,
                rarity: 0,
                buffs: Vec::new(),
            },
        }),
        WorldEntityKind::Npc => Some(ServerPacket::ObjectNpc {
            info: NpcInfo {
                object_id: entity.object_id,
                name: entity.name.clone(),
                name_colour_argb: entity.name_colour_argb,
                image: sprite_image_from_shared_entity(entity, "NPC"),
                colour_argb: -1,
                location,
                direction: entity.direction,
                quest_ids: entity.quest_ids.clone(),
            },
        }),
        WorldEntityKind::SelfPlayer | WorldEntityKind::Player => None,
    }
}

fn zone_monster_spawn_packet(spawn: &ZoneMonsterSpawn) -> ServerPacket {
    ServerPacket::ObjectMonster {
        info: MonsterInfo {
            object_id: spawn.object_id,
            name: spawn.name.clone(),
            name_colour_argb: spawn.name_colour_argb,
            location: spawn.position.clone(),
            image: spawn.image,
            direction: spawn.direction,
            effect: 0,
            ai: spawn.ai,
            light: 0,
            dead: spawn.hp <= 0,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id: 0,
            rarity: 0,
            buffs: Vec::new(),
        },
    }
}

fn shared_npc_entity_side_effect_packets(
    before: &WorldSnapshot,
    after: &WorldSnapshot,
    current_tick: u64,
) -> Vec<ServerPacket> {
    if before.map_file_name.as_deref() != after.map_file_name.as_deref() {
        return Vec::new();
    }

    let before_monsters = before
        .entities
        .iter()
        .filter(|entity| entity.kind == WorldEntityKind::Monster)
        .map(|entity| (entity.object_id, entity))
        .collect::<BTreeMap<_, _>>();
    let after_monsters = after
        .entities
        .iter()
        .filter(|entity| entity.kind == WorldEntityKind::Monster)
        .map(|entity| (entity.object_id, entity))
        .collect::<BTreeMap<_, _>>();
    let mut packets = Vec::new();

    for (object_id, entity) in &after_monsters {
        let is_dead = entity.dead || entity.hp.is_some_and(|hp| hp <= 0);
        let Some(before_entity) = before_monsters.get(object_id) else {
            if !is_dead {
                if let Some(spawn) = zone_monster_spawn_from_shared_entity(entity, current_tick) {
                    packets.push(zone_monster_spawn_packet(&spawn));
                } else if let Some(packet) = shared_entity_spawn_packet(entity) {
                    packets.push(packet);
                }
            }
            continue;
        };
        let was_dead = before_entity.dead || before_entity.hp.is_some_and(|hp| hp <= 0);
        if !was_dead && is_dead {
            packets.push(ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: *object_id,
                    percent: 0,
                    expire: 0,
                },
            });
            packets.push(ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: *object_id,
                    location: Point {
                        x: entity.x,
                        y: entity.y,
                    },
                    direction: entity.direction,
                    kind: 0,
                },
            });
        }
    }

    for (object_id, entity) in before_monsters {
        if !after_monsters.contains_key(&object_id)
            && !entity.dead
            && !entity.hp.is_some_and(|hp| hp <= 0)
        {
            packets.push(ServerPacket::ObjectRemove { object_id });
        }
    }

    packets
}

fn shared_npc_entity_side_effect_matches(
    expected: Option<&SharedNpcEntitySideEffect>,
    actual: Option<&SharedNpcEntitySideEffect>,
) -> bool {
    match (expected, actual) {
        (None, None) => true,
        (Some(expected), Some(actual)) => {
            expected.map_file_name == actual.map_file_name && expected.packets == actual.packets
        }
        _ => false,
    }
}

const SHARED_DEATH_DROP_RANGE: i32 = 4;
const SHARED_CRYSTAL_TICK_MS: u64 = 300;
const SHARED_DROP_EXPIRE_TICKS: u64 = 30 * 60;

fn shared_gateway_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn normalize_gateway_map_file_name(file_name: &str) -> String {
    file_name
        .trim()
        .trim_end_matches(".map")
        .trim_end_matches(".MAP")
        .to_ascii_lowercase()
}

fn point_in_direction(point: &Point, direction: MirDirection) -> Point {
    let (dx, dy) = match direction {
        MirDirection::Up => (0, -1),
        MirDirection::UpRight => (1, -1),
        MirDirection::Right => (1, 0),
        MirDirection::DownRight => (1, 1),
        MirDirection::Down => (0, 1),
        MirDirection::DownLeft => (-1, 1),
        MirDirection::Left => (-1, 0),
        MirDirection::UpLeft => (-1, -1),
    };
    Point {
        x: point.x + dx,
        y: point.y + dy,
    }
}

fn direction_toward_points(from: &Point, to: &Point) -> Option<MirDirection> {
    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();
    match (dx, dy) {
        (0, -1) => Some(MirDirection::Up),
        (1, -1) => Some(MirDirection::UpRight),
        (1, 0) => Some(MirDirection::Right),
        (1, 1) => Some(MirDirection::DownRight),
        (0, 1) => Some(MirDirection::Down),
        (-1, 1) => Some(MirDirection::DownLeft),
        (-1, 0) => Some(MirDirection::Left),
        (-1, -1) => Some(MirDirection::UpLeft),
        _ => None,
    }
}

fn zone_magic_launch_accepted(packets: &[ServerPacket], spell: Spell, object_id: u32) -> bool {
    packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::Magic {
                spell: packet_spell,
                target_id,
                cast: true,
                ..
            } if *packet_spell == spell && *target_id == object_id
        )
    })
}

fn gateway_zone_magic_targets_ground(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::FireWall
            | Spell::Blizzard
            | Spell::MeteorStrike
            | Spell::PoisonCloud
            | Spell::ExplosiveTrap
    )
}

fn gateway_zone_magic_targets_summon(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::SummonSkeleton
            | Spell::SummonShinsu
            | Spell::SummonHolyDeva
            | Spell::SummonVampire
            | Spell::SummonToad
            | Spell::SummonSnakes
            | Spell::Stonetrap
    )
}

fn gateway_zone_magic_targets_self(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::Healing | Spell::MassHealing | Spell::HealingCircle | Spell::MagicShield
    )
}

fn gateway_zone_magic_requires_item_consumption(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::PoisonCloud | Spell::SummonSkeleton | Spell::SummonShinsu | Spell::SummonHolyDeva
    )
}

fn gateway_zone_magic_item_damage_bonus(spell: Spell) -> i32 {
    match spell {
        Spell::PoisonCloud => 1,
        _ => 0,
    }
}

fn death_drop_anchors(
    map: &ZoneMapSnapshotLayer,
    packets: &[ServerPacket],
) -> Vec<(u32, Option<String>, Point)> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectDied { info } => Some((
                info.object_id,
                map.entities
                    .get(&info.object_id)
                    .map(|entity| entity.name.clone()),
                info.location.clone(),
            )),
            ServerPacket::ObjectHealth { info } if info.percent == 0 => map
                .entities
                .get(&info.object_id)
                .map(|entity| {
                    (
                        info.object_id,
                        Some(entity.name.clone()),
                        Point {
                            x: entity.x,
                            y: entity.y,
                        },
                    )
                })
                .or_else(|| {
                    map.dead_entity_ids
                        .get(&info.object_id)
                        .and_then(|dead| dead.location.clone())
                        .map(|location| (info.object_id, None, location))
                }),
            _ => None,
        })
        .collect()
}

fn drop_matches_committed_death(map: &ZoneMapSnapshotLayer, drop: &GroundDropSnapshot) -> bool {
    map.committed_death_drop_anchors.values().any(|anchor| {
        drop_matches_death_anchor(drop, anchor.monster_name.as_deref(), &anchor.location)
    })
}

fn drop_matches_death_anchor(
    drop: &GroundDropSnapshot,
    monster_name: Option<&str>,
    location: &Point,
) -> bool {
    if drop.source_monster.is_empty() {
        return false;
    }
    if let Some(monster_name) = monster_name {
        if drop.source_monster != monster_name {
            return false;
        }
    }
    (drop.x - location.x).abs() <= SHARED_DEATH_DROP_RANGE
        && (drop.y - location.y).abs() <= SHARED_DEATH_DROP_RANGE
}

fn ground_drop_spawn_packet(drop: &GroundDropSnapshot) -> ServerPacket {
    let location = Point {
        x: drop.x,
        y: drop.y,
    };
    match &drop.loot {
        GroundDropLootSnapshot::Gold { amount } => ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: drop.object_id,
                gold: *amount,
                location,
            },
        },
        GroundDropLootSnapshot::InventoryItem { .. } => ServerPacket::ObjectItem {
            info: ObjectItemInfo {
                object_id: drop.object_id,
                name: drop.name.clone(),
                name_colour_argb: drop.name_colour_argb,
                location,
                image: drop.icon,
                grade: 0,
            },
        },
    }
}

fn ground_drop_spawn_object_ids(packets: &[ServerPacket]) -> BTreeSet<u32> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectItem { info } => Some(info.object_id),
            ServerPacket::ObjectGold { info } => Some(info.object_id),
            _ => None,
        })
        .collect()
}

fn remove_object_remove_packets(packets: &mut Vec<ServerPacket>, object_ids: &BTreeSet<u32>) {
    if object_ids.is_empty() {
        return;
    }
    packets.retain(|packet| {
        !matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if object_ids.contains(object_id)
        )
    });
}

fn apply_shared_entity_transform(
    map: &mut ZoneMapSnapshotLayer,
    object_id: u32,
    location: &Point,
    direction: MirDirection,
) {
    if map.dead_entity_ids.contains_key(&object_id) {
        return;
    }
    if let Some(entity) = map.entities.get_mut(&object_id) {
        if entity.dead || entity.hp.is_some_and(|hp| hp <= 0) {
            return;
        }
        entity.x = location.x;
        entity.y = location.y;
        entity.direction = direction;
    }
}

fn shared_entity_observer_packet_object_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement } => Some(movement.object_id),
        ServerPacket::ObjectAttack { info } => Some(info.object_id),
        ServerPacket::ObjectRangeAttack { info } => Some(info.object_id),
        ServerPacket::ObjectMagic { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectProjectile { source_id, .. } => Some(*source_id),
        ServerPacket::ObjectStruck { info } => Some(info.attacker_id),
        ServerPacket::ObjectHealth { info } => Some(info.object_id),
        ServerPacket::ObjectDied { info } => Some(info.object_id),
        ServerPacket::ObjectRemove { object_id } => Some(*object_id),
        _ => None,
    }
}

fn coalesced_zone_movement_object_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement } => Some(movement.object_id),
        _ => None,
    }
}

fn shared_entity_target_result_packet_object_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectHealth { info } => Some(info.object_id),
        ServerPacket::DamageIndicator { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectDied { info } => Some(info.object_id),
        ServerPacket::ObjectPoisoned { object_id, .. } => Some(*object_id),
        ServerPacket::AddBuff { buff } => Some(buff.object_id),
        ServerPacket::RemoveBuff { object_id, .. } | ServerPacket::PauseBuff { object_id, .. } => {
            Some(*object_id)
        }
        _ => None,
    }
}

fn delayed_player_action_packets(
    owner_local_object_id: u32,
    packets: &[ServerPacket],
) -> Vec<ServerPacket> {
    let struck_object_ids = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectStruck { info } if info.attacker_id == owner_local_object_id => {
                Some(info.object_id)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if struck_object_ids.is_empty() {
        return Vec::new();
    }
    packets
        .iter()
        .filter(|packet| match packet {
            ServerPacket::ObjectStruck { info } => info.attacker_id == owner_local_object_id,
            ServerPacket::ObjectHealth { info } => {
                struck_object_ids.contains(&info.object_id)
                    || info.object_id == owner_local_object_id
            }
            ServerPacket::DamageIndicator { object_id, .. } => {
                struck_object_ids.contains(object_id) || *object_id == owner_local_object_id
            }
            ServerPacket::ObjectDied { info } => struck_object_ids.contains(&info.object_id),
            ServerPacket::ObjectPoisoned { object_id, .. } => {
                struck_object_ids.contains(object_id) || *object_id == owner_local_object_id
            }
            ServerPacket::AddBuff { buff } => {
                struck_object_ids.contains(&buff.object_id)
                    || buff.object_id == owner_local_object_id
            }
            ServerPacket::RemoveBuff { object_id, .. }
            | ServerPacket::PauseBuff { object_id, .. } => {
                struck_object_ids.contains(object_id) || *object_id == owner_local_object_id
            }
            ServerPacket::ObjectRemove { object_id } => struck_object_ids.contains(object_id),
            ServerPacket::ObjectItem { .. } | ServerPacket::ObjectGold { .. } => true,
            _ => false,
        })
        .cloned()
        .collect()
}

fn shared_trade_offer_fits(free_bag_slots: u16, offer: &SharedTradeOffer) -> bool {
    usize::from(free_bag_slots) >= offer.items.len()
}

impl ZoneRuntimeFactory for SharedInProcessZoneRuntimeFactory {
    fn create_runtime(&self, config: GatewayConfig, zone_id: &ZoneId) -> ZoneRuntimeHandle {
        Box::new(SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(config),
            zone_state: self.state_for_zone(zone_id),
            account_inventory_service: self.account_inventory_service.clone(),
            npc_world_service: self.npc_world_service.clone(),
            presence_key: None,
            zone_move_seq: 0,
            shared_skill_item_request_seq: 0,
            force_next_zone_transform_sync: false,
            cached_map_file_name: None,
            cached_local_self_object_id: None,
            cached_map_transfers: Vec::new(),
            last_shared_entity_ids_by_map: BTreeMap::new(),
            last_shared_drop_ids_by_map: BTreeMap::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedMapTransfer {
    key: String,
    map_file_name: String,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

struct SharedInProcessZoneSessionRuntime {
    inner: InProcessWorldRuntime,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    account_inventory_service: SharedAccountInventoryServiceHandle,
    npc_world_service: SharedNpcWorldServiceHandle,
    presence_key: Option<ZonePresenceKey>,
    zone_move_seq: u64,
    shared_skill_item_request_seq: u64,
    force_next_zone_transform_sync: bool,
    cached_map_file_name: Option<String>,
    cached_local_self_object_id: Option<u32>,
    cached_map_transfers: Vec<CachedMapTransfer>,
    last_shared_entity_ids_by_map: BTreeMap<String, BTreeSet<u32>>,
    last_shared_drop_ids_by_map: BTreeMap<String, BTreeSet<u32>>,
}

impl fmt::Debug for SharedInProcessZoneSessionRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedInProcessZoneSessionRuntime")
            .field("inner", &"InProcessWorldRuntime")
            .field("presence_key", &self.presence_key)
            .finish()
    }
}

impl SharedInProcessZoneSessionRuntime {
    fn zone_now_ms() -> u64 {
        shared_gateway_now_ms()
    }

    fn next_shared_skill_item_request_id(&mut self) -> u64 {
        self.shared_skill_item_request_seq = self.shared_skill_item_request_seq.saturating_add(1);
        self.shared_skill_item_request_seq
    }

    fn apply_zone_transform(&mut self, transform: Option<(Point, MirDirection)>) {
        if let Some((position, direction)) = transform {
            self.inner
                .force_authoritative_player_transform(position, direction);
        }
    }

    fn apply_zone_shout_consume(&mut self, consume: Option<(bool, bool)>) {
        if let Some((map_shout, server_shout)) = consume {
            self.inner
                .consume_zone_chat_shout_permission(map_shout, server_shout);
        }
    }

    fn current_zone_transfer_key(&self) -> Option<String> {
        let normalized_map = normalize_gateway_map_file_name(self.cached_map_file_name.as_deref()?);
        let key = self.current_presence_key()?;
        let position = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .players
            .get(&key)
            .map(|presence| Point {
                x: presence.entity.x,
                y: presence.entity.y,
            })?;

        self.cached_map_transfers
            .iter()
            .find(|transfer| {
                normalize_gateway_map_file_name(&transfer.map_file_name) == normalized_map
                    && position.x >= transfer.min_x
                    && position.x <= transfer.max_x
                    && position.y >= transfer.min_y
                    && position.y <= transfer.max_y
            })
            .map(|transfer| transfer.key.clone())
    }

    fn apply_zone_current_position_map_transfer(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_zone_transfer_key() else {
            return Vec::new();
        };

        let mut packets = match self.inner.execute(WorldCommand::TransferMap { key }) {
            Ok(packets) => packets,
            Err(_) => return Vec::new(),
        };
        if packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MapInformation { .. }))
        {
            self.force_next_zone_transform_sync = true;
            packets.extend(self.sync_zone_snapshot());
        }
        packets
    }

    fn sync_zone_snapshot(&mut self) -> Vec<ServerPacket> {
        let allow_transform_sync = self.force_next_zone_transform_sync;
        self.force_next_zone_transform_sync = false;
        let Some(identity) = self.inner.active_identity() else {
            return self.remove_presence();
        };
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.clone() else {
            return self.remove_presence();
        };
        self.cached_map_file_name = Some(map_file_name.clone());
        self.cached_map_transfers = snapshot
            .map_transfers
            .iter()
            .map(|transfer| CachedMapTransfer {
                key: transfer.key.clone(),
                map_file_name: transfer.map_file_name.clone(),
                min_x: transfer.bounds.min_x,
                max_x: transfer.bounds.max_x,
                min_y: transfer.bounds.min_y,
                max_y: transfer.bounds.max_y,
            })
            .collect();

        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .cloned();
        let shared_entities = snapshot
            .entities
            .iter()
            .filter(|entity| matches!(entity.kind, WorldEntityKind::Monster | WorldEntityKind::Npc))
            .cloned()
            .collect::<Vec<_>>();
        let native_monster_spawns = shared_entities
            .iter()
            .filter(|entity| entity.kind == WorldEntityKind::Monster && !entity.dead)
            .filter_map(|entity| self.inner.zone_monster_spawn_snapshot(entity.object_id))
            .collect::<Vec<_>>();
        let map_hazard_config = self.inner.current_map_hazard_config();
        let shared_entity_ids = shared_entities
            .iter()
            .map(|entity| entity.object_id)
            .collect::<BTreeSet<_>>();
        let previous_entity_ids = self
            .last_shared_entity_ids_by_map
            .insert(map_file_name.clone(), shared_entity_ids)
            .unwrap_or_default();

        let mut shared_ground_drops = snapshot.ground_drops.clone();
        let shared_drop_ids = shared_ground_drops
            .iter()
            .map(|drop| drop.object_id)
            .collect::<BTreeSet<_>>();
        let previous_drop_ids = self
            .last_shared_drop_ids_by_map
            .insert(map_file_name.clone(), shared_drop_ids)
            .unwrap_or_default();

        let Some(self_entity) = self_entity else {
            return self.remove_presence();
        };
        let key = ZonePresenceKey::from_identity(&identity);
        let session_id = SharedInProcessZoneState::zone_session_id_for_key(&key);
        let mut join_snapshot = self
            .inner
            .active_zone_join_snapshot(session_id.as_str().to_string());
        let mut packets = Vec::new();
        let mut transform = None;
        let mut shout_consume = None;
        let mut player_damages = Vec::new();
        let mut player_heals = Vec::new();
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        let previous_map = zone_state
            .players
            .get(&key)
            .map(|presence| presence.map_file_name.clone());
        if let Some(previous_map_name) = previous_map
            .as_deref()
            .filter(|previous_map_name| *previous_map_name != map_file_name.as_str())
        {
            let owner_name = zone_state
                .players
                .get(&key)
                .map(|presence| presence.entity.name.clone())
                .unwrap_or_else(|| identity.character_name.clone());
            zone_state.remove_owned_shared_entities(&owner_name, previous_map_name, Some(&key));
        }
        let zone_object_id = zone_state.upsert_player(
            key.clone(),
            &identity.character_name,
            map_file_name.clone(),
            self_entity,
            snapshot.free_bag_slots,
        );
        let local_self_object_id = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| entity.object_id);
        self.cached_local_self_object_id = local_self_object_id;
        if let Some(local_self_object_id) = local_self_object_id {
            for drop in &mut shared_ground_drops {
                if drop.owner_object_id == Some(local_self_object_id) {
                    drop.owner_object_id = Some(zone_object_id);
                }
            }
        }
        zone_state.sync_map_layer(
            map_file_name.clone(),
            shared_entities,
            previous_entity_ids,
            shared_ground_drops,
            previous_drop_ids,
        );
        let zone_ground_drops = zone_state
            .maps
            .get(&map_file_name)
            .map(|map| map.ground_drops.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let should_join_zone = zone_state.zone_sessions.get(&key).is_none()
            || previous_map.as_deref() != Some(map_file_name.as_str());
        if should_join_zone {
            if let Some(join) = join_snapshot.as_mut() {
                join.object_id = zone_object_id;
                join.map_file_name = map_file_name.clone();
                zone_state
                    .zone_sessions
                    .insert(key.clone(), session_id.clone());
                zone_state
                    .zone_session_keys
                    .insert(session_id.clone(), key.clone());
                let mut outbounds = zone_state.zone_manager.join(join.clone());
                let now_ms = Self::zone_now_ms();
                outbounds.extend(
                    zone_state
                        .zone_manager
                        .handle(ZoneCommand::SyncGroundDrops {
                            session_id: session_id.clone(),
                            drops: zone_ground_drops,
                            now_ms,
                        }),
                );
                for monster in &native_monster_spawns {
                    outbounds.extend(zone_state.zone_manager.handle(ZoneCommand::SpawnMonster {
                        session_id: session_id.clone(),
                        monster: monster.clone(),
                        now_ms,
                    }));
                }
                if let Some((lightning, fire, lightning_damage, fire_damage)) = map_hazard_config {
                    outbounds.extend(zone_state.zone_manager.handle(
                        ZoneCommand::ConfigureHazards {
                            session_id: session_id.clone(),
                            lightning,
                            fire,
                            lightning_damage,
                            fire_damage,
                        },
                    ));
                }
                let (
                    zone_packets,
                    zone_transform,
                    zone_shout_consume,
                    _,
                    _,
                    zone_player_damages,
                    zone_player_heals,
                ) = zone_state.dispatch_zone_outbounds(outbounds, Some(&key));
                packets.extend(zone_packets);
                transform = zone_transform;
                shout_consume = zone_shout_consume;
                player_damages = zone_player_damages;
                player_heals = zone_player_heals;
            }
        } else if let Some(join) = join_snapshot.as_ref() {
            let mut outbounds = Vec::new();
            let now_ms = Self::zone_now_ms();
            let zone_transform = zone_state.zone_manager.player_transform(&session_id);
            if allow_transform_sync
                && zone_transform.is_some_and(|(position, direction)| {
                    position != join.position || direction != join.direction
                })
            {
                outbounds.extend(zone_state.zone_manager.handle(
                    ZoneCommand::SyncPlayerTransform {
                        session_id: session_id.clone(),
                        position: join.position.clone(),
                        direction: join.direction,
                    },
                ));
            }
            outbounds.extend(
                zone_state
                    .zone_manager
                    .handle(ZoneCommand::UpdateChatProfile {
                        session_id: session_id.clone(),
                        profile: join.chat_profile.clone(),
                    }),
            );
            // Keep the zone's authoritative combat view fresh so equipment,
            // buff, and level changes are reflected the next time the zone rolls
            // this player's damage.
            outbounds.extend(zone_state.zone_manager.handle(
                ZoneCommand::UpdatePlayerCombatStats {
                    session_id: session_id.clone(),
                    stats: join.combat_stats,
                },
            ));
            outbounds.extend(
                zone_state
                    .zone_manager
                    .handle(ZoneCommand::SyncGroundDrops {
                        session_id: session_id.clone(),
                        drops: zone_ground_drops,
                        now_ms,
                    }),
            );
            for monster in &native_monster_spawns {
                outbounds.extend(zone_state.zone_manager.handle(ZoneCommand::SpawnMonster {
                    session_id: session_id.clone(),
                    monster: monster.clone(),
                    now_ms,
                }));
            }
            let (
                zone_packets,
                zone_transform,
                zone_shout_consume,
                _,
                _,
                zone_player_damages,
                zone_player_heals,
            ) = zone_state.dispatch_zone_outbounds(outbounds, Some(&key));
            packets.extend(zone_packets);
            transform = zone_transform;
            shout_consume = zone_shout_consume;
            player_damages = zone_player_damages;
            player_heals = zone_player_heals;
        }
        self.presence_key = Some(key);
        drop(zone_state);
        self.apply_zone_player_buff_packets(&packets);
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        self.apply_zone_player_damages(player_damages);
        self.apply_zone_player_heals(player_heals);
        packets
    }

    fn remove_presence(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.presence_key.take() else {
            return Vec::new();
        };
        self.cached_local_self_object_id = None;
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        let outbounds = zone_state.remove_player(&key);
        let (packets, transform, shout_consume, _, _, player_damages, player_heals) =
            zone_state.dispatch_zone_outbounds(outbounds, Some(&key));
        zone_state.forget_zone_session(&key);
        drop(zone_state);
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        self.apply_zone_player_damages(player_damages);
        self.apply_zone_player_heals(player_heals);
        packets
    }

    fn current_presence_key(&self) -> Option<ZonePresenceKey> {
        self.presence_key.clone().or_else(|| {
            self.inner
                .active_identity()
                .map(|identity| ZonePresenceKey {
                    account_id: identity.account_id,
                    character_index: identity.character_index,
                })
        })
    }

    fn current_zone_session_id(&self) -> Option<SessionId> {
        self.current_presence_key()
            .map(|key| SharedInProcessZoneState::zone_session_id_for_key(&key))
    }

    fn apply_pending_zone_packets(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let (
            mut packets,
            transform,
            shout_consume,
            ground_drop_claims,
            monster_kill_awards,
            player_damages,
            player_heals,
        ) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            (
                zone_state.take_pending_zone_packets(&key),
                zone_state.take_pending_zone_transform(&key),
                zone_state.take_pending_zone_shout_consume(&key),
                zone_state.take_pending_zone_ground_drop_claims(&key),
                zone_state.take_pending_zone_monster_kill_awards(&key),
                zone_state.take_pending_zone_player_damages(&key),
                zone_state.take_pending_zone_player_heals(&key),
            )
        };
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        self.apply_zone_player_damages(player_damages);
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&packets);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets, canceled_claims) =
            self.apply_zone_ground_drop_claims(ground_drop_claims);
        remove_object_remove_packets(&mut packets, &canceled_claims);
        packets.extend(claim_packets);
        packets
    }

    fn expire_shared_drops_on_current_map(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let Some(map_file_name) = self
            .cached_map_file_name
            .clone()
            .or_else(|| self.inner.world_snapshot().map_file_name)
        else {
            return Vec::new();
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .expire_shared_drops(&map_file_name, Some(&key), Self::zone_now_ms())
    }

    fn dispatch_zone_player_command(
        &mut self,
        command: ZoneCommand,
        tick_after_command: bool,
    ) -> Vec<ServerPacket> {
        let (mut packets, ground_drop_claims, monster_kill_awards, player_damages, player_heals) =
            self.dispatch_zone_player_command_collecting_claims(command, tick_after_command);
        self.apply_zone_player_damages(player_damages);
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&packets);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets, canceled_claims) =
            self.apply_zone_ground_drop_claims(ground_drop_claims);
        remove_object_remove_packets(&mut packets, &canceled_claims);
        packets.extend(claim_packets);
        packets
    }

    fn dispatch_zone_player_command_collecting_claims(
        &mut self,
        command: ZoneCommand,
        tick_after_command: bool,
    ) -> (
        Vec<ServerPacket>,
        Vec<GroundDropSnapshot>,
        Vec<ZoneMonsterKillAward>,
        Vec<i32>,
        Vec<i32>,
    ) {
        let mut commands = vec![command];
        if tick_after_command {
            commands.push(ZoneCommand::Tick {
                now_ms: Self::zone_now_ms(),
            });
        }
        self.dispatch_zone_player_commands_collecting_claims(commands)
    }

    fn dispatch_zone_player_commands_collecting_claims(
        &mut self,
        commands: Vec<ZoneCommand>,
    ) -> (
        Vec<ServerPacket>,
        Vec<GroundDropSnapshot>,
        Vec<ZoneMonsterKillAward>,
        Vec<i32>,
        Vec<i32>,
    ) {
        let Some(key) = self.current_presence_key() else {
            return (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        };
        let (
            packets,
            transform,
            shout_consume,
            ground_drop_claims,
            monster_kill_awards,
            player_damages,
            player_heals,
        ) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let mut outbounds = Vec::new();
            for command in commands {
                outbounds.extend(zone_state.zone_manager.handle(command));
            }
            zone_state.dispatch_zone_outbounds(outbounds, Some(&key))
        };
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        (
            packets,
            ground_drop_claims,
            monster_kill_awards,
            player_damages,
            player_heals,
        )
    }

    fn apply_zone_player_damages(&mut self, damages: Vec<i32>) {
        for damage in damages {
            self.inner.apply_zone_player_damage(damage);
        }
    }

    fn apply_zone_player_heals(&mut self, heals: Vec<i32>) {
        for amount in heals {
            self.inner.apply_zone_player_heal(amount);
        }
    }

    fn apply_zone_player_buff_packets(&mut self, packets: &[ServerPacket]) {
        let Some(zone_object_id) = self.current_zone_player_object_id() else {
            return;
        };
        self.inner
            .apply_zone_player_buff_packets(packets, zone_object_id);
    }

    fn current_zone_player_object_id(&self) -> Option<u32> {
        let key = self.current_presence_key()?;
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .players
            .get(&key)
            .map(|presence| presence.zone_object_id)
    }

    fn apply_zone_monster_kill_awards(
        &mut self,
        awards: Vec<ZoneMonsterKillAward>,
    ) -> Vec<ServerPacket> {
        let Some(identity) = self.inner.active_identity() else {
            return Vec::new();
        };
        awards
            .into_iter()
            .flat_map(|award| {
                let receipt = self.account_inventory_service.commit(
                    &mut self.inner,
                    SharedAccountInventoryCommandEnvelope {
                        identity: identity.clone(),
                        command: SharedAccountInventoryCommand::MonsterKillAward(award),
                    },
                );
                debug_assert_eq!(
                    receipt.kind,
                    SharedAccountInventoryTransactionKind::MonsterKillAward
                );
                receipt.packets
            })
            .collect()
    }

    fn apply_zone_ground_drop_claims(
        &mut self,
        claims: Vec<GroundDropSnapshot>,
    ) -> (Vec<ServerPacket>, BTreeSet<u32>) {
        if claims.is_empty() {
            return (Vec::new(), BTreeSet::new());
        }
        let Some(identity) = self.inner.active_identity() else {
            return (Vec::new(), BTreeSet::new());
        };
        let Some(session_id) = self.current_zone_session_id() else {
            return (Vec::new(), BTreeSet::new());
        };
        let mut packets = Vec::new();
        let mut canceled_claims = BTreeSet::new();
        for drop in claims {
            let object_id = drop.object_id;
            let mut receipt = self.account_inventory_service.commit(
                &mut self.inner,
                SharedAccountInventoryCommandEnvelope {
                    identity: identity.clone(),
                    command: SharedAccountInventoryCommand::GroundDropPickup(drop.clone()),
                },
            );
            debug_assert_eq!(
                receipt.kind,
                SharedAccountInventoryTransactionKind::GroundDropPickup
            );
            let followup = if receipt.committed {
                ZoneCommand::CommitGroundDropClaim {
                    session_id: session_id.clone(),
                    object_id,
                }
            } else {
                self.restore_zone_ground_drop_claim(drop.clone());
                canceled_claims.insert(object_id);
                ZoneCommand::CancelGroundDropClaim {
                    session_id: session_id.clone(),
                    object_id,
                    now_ms: Self::zone_now_ms(),
                }
            };
            receipt
                .packets
                .extend(self.dispatch_zone_player_command(followup, false));
            packets.extend(receipt.packets);
        }
        (packets, canceled_claims)
    }

    fn restore_zone_ground_drop_claim(&mut self, drop: GroundDropSnapshot) {
        let Some(key) = self.current_presence_key() else {
            return;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .restore_drop_for_key(&key, drop);
    }

    fn local_self_object_id(&self) -> Option<u32> {
        if let Some(object_id) = self.cached_local_self_object_id {
            return Some(object_id);
        }
        self.inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| entity.object_id)
    }

    fn dispatch_zone_observer_packets(
        &mut self,
        owner_local_object_id: u32,
        packets: &[ServerPacket],
    ) -> Vec<ServerPacket> {
        if packets.is_empty() {
            return Vec::new();
        }
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        self.dispatch_zone_player_command(
            ZoneCommand::BroadcastPackets {
                session_id,
                owner_local_object_id,
                packets: packets.to_vec(),
                now_ms: Self::zone_now_ms(),
            },
            false,
        )
    }

    fn dispatch_shared_quest_share_packets(
        &mut self,
        packets: &[ServerPacket],
    ) -> Vec<ServerPacket> {
        let quest_share_packets = packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::ShareQuest { .. }))
            .cloned()
            .collect::<Vec<_>>();
        if quest_share_packets.is_empty() {
            return Vec::new();
        }
        let Some(current_key) = self.current_presence_key() else {
            return Vec::new();
        };
        let Some(identity) = self.inner.active_identity() else {
            return Vec::new();
        };
        let target_names = self
            .inner
            .world_snapshot()
            .stage5_systems
            .group
            .members
            .into_iter()
            .filter(|name| !name.eq_ignore_ascii_case(&identity.character_name))
            .collect::<Vec<_>>();
        if target_names.is_empty() {
            return Vec::new();
        }
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .queue_zone_packets_for_player_names(&target_names, &current_key, quest_share_packets)
    }

    fn dispatch_shared_entity_observer_packets(&mut self, packets: &[ServerPacket]) {
        let observer_object_ids = packets
            .iter()
            .filter_map(shared_entity_observer_packet_object_id)
            .collect::<BTreeSet<_>>();
        if observer_object_ids.is_empty() {
            return;
        }
        let Some(session_id) = self.current_zone_session_id() else {
            return;
        };
        let Some(current_key) = self.current_presence_key() else {
            return;
        };
        let local_self_object_id = self.local_self_object_id();
        let (seed_packets, observer_packets) = {
            let zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let Some(current_presence) = zone_state.players.get(&current_key).cloned() else {
                return;
            };
            let map_file_name = current_presence.map_file_name;
            let shared_entity_hits_local_self = local_self_object_id.is_some_and(|local_self| {
                packets.iter().any(|packet| match packet {
                    ServerPacket::ObjectStruck { info } if info.object_id == local_self => {
                        observer_object_ids.contains(&info.attacker_id)
                            && zone_state
                                .shared_entity(&map_file_name, info.attacker_id)
                                .is_some()
                    }
                    _ => false,
                })
            });
            let seed_packets = observer_object_ids
                .iter()
                .filter_map(|object_id| zone_state.shared_entity(&map_file_name, *object_id))
                .filter_map(|entity| shared_entity_spawn_packet(&entity))
                .collect::<Vec<_>>();
            let observer_packets = packets
                .iter()
                .filter(|packet| {
                    let shared_actor_packet = shared_entity_observer_packet_object_id(packet)
                        .is_some_and(|object_id| {
                            observer_object_ids.contains(&object_id)
                                && zone_state
                                    .shared_entity(&map_file_name, object_id)
                                    .is_some()
                        });
                    let local_self_result_packet = shared_entity_hits_local_self
                        && shared_entity_target_result_packet_object_id(packet)
                            == local_self_object_id;
                    shared_actor_packet || local_self_result_packet
                })
                .cloned()
                .collect::<Vec<_>>();
            (seed_packets, observer_packets)
        };
        if !seed_packets.is_empty() {
            let _ = self.dispatch_zone_player_command(
                ZoneCommand::SyncSharedObjects {
                    session_id: session_id.clone(),
                    packets: seed_packets,
                    now_ms: Self::zone_now_ms(),
                },
                false,
            );
        }
        if !observer_packets.is_empty() {
            let _ = self.dispatch_zone_player_command(
                ZoneCommand::BroadcastSharedObjectPackets {
                    session_id,
                    local_self_object_id,
                    packets: observer_packets,
                    now_ms: Self::zone_now_ms(),
                },
                false,
            );
        }
    }

    fn current_snapshot_ground_drops_for_shared_state(
        snapshot: &WorldSnapshot,
        zone_state: &SharedInProcessZoneState,
        current_key: Option<&ZonePresenceKey>,
    ) -> Vec<GroundDropSnapshot> {
        let local_self_object_id = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| entity.object_id);
        let zone_object_id = current_key.and_then(|key| {
            zone_state
                .players
                .get(key)
                .map(|presence| presence.zone_object_id)
        });
        let mut drops = snapshot.ground_drops.clone();
        if let (Some(local_self_object_id), Some(zone_object_id)) =
            (local_self_object_id, zone_object_id)
        {
            for drop in &mut drops {
                if drop.owner_object_id == Some(local_self_object_id) {
                    drop.owner_object_id = Some(zone_object_id);
                }
            }
        }
        drops
    }

    fn apply_shared_entity_packets_to_current_map(&mut self, packets: &[ServerPacket]) {
        if packets.is_empty() {
            return;
        }
        let map_file_name = self
            .cached_map_file_name
            .clone()
            .or_else(|| self.inner.world_snapshot().map_file_name);
        if let Some(map_file_name) = map_file_name.as_deref() {
            let current_key = self.current_presence_key();
            let local_self_object_id = self.local_self_object_id();
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let zone_object_id = current_key
                .as_ref()
                .and_then(|key| zone_state.players.get(key))
                .map(|presence| presence.zone_object_id);
            if let (Some(local_self_object_id), Some(zone_object_id)) =
                (local_self_object_id, zone_object_id)
            {
                let packets = packets
                    .iter()
                    .map(|packet| match packet {
                        ServerPacket::ObjectMonster { info }
                            if info.master_object_id == local_self_object_id =>
                        {
                            let mut info = info.clone();
                            info.master_object_id = zone_object_id;
                            ServerPacket::ObjectMonster { info }
                        }
                        packet => packet.clone(),
                    })
                    .collect::<Vec<_>>();
                zone_state.apply_shared_entity_packets(map_file_name, &packets);
            } else {
                zone_state.apply_shared_entity_packets(map_file_name, packets);
            }
        }
    }

    fn commit_shared_death_drops_to_current_map(
        &mut self,
        packets: &[ServerPacket],
    ) -> Vec<ServerPacket> {
        if packets.is_empty() {
            return Vec::new();
        }
        if !packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectDied { .. }
                    | ServerPacket::ObjectHealth {
                        info: ObjectHealthInfo { percent: 0, .. },
                    }
            )
        }) {
            return Vec::new();
        }
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return Vec::new();
        };
        let current_key = self.current_presence_key();
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        let ground_drops = Self::current_snapshot_ground_drops_for_shared_state(
            &snapshot,
            &zone_state,
            current_key.as_ref(),
        );
        let existing_drop_object_ids = ground_drop_spawn_object_ids(packets);
        zone_state
            .commit_death_drops(map_file_name, packets, &ground_drops)
            .into_iter()
            .filter(|drop| !existing_drop_object_ids.contains(&drop.object_id))
            .map(|drop| ground_drop_spawn_packet(&drop))
            .collect()
    }

    fn shared_action_target_available(&self, object_id: u32) -> bool {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return true;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_entity_allows_action(map_file_name, object_id)
    }

    fn prepare_zone_native_player_attack(
        &self,
        command: &WorldCommand,
    ) -> Option<ZoneNativePlayerAttack> {
        let (object_id, packet_direction, requested_target, mut kind) = match command {
            WorldCommand::Attack { object_id } => (
                *object_id,
                None,
                None,
                ZoneNativePlayerAttackKind::Melee {
                    spell: Spell::None as u8,
                    attack_type: 0,
                },
            ),
            WorldCommand::ClientPacket(ClientPacket::RangeAttack {
                direction,
                target_id,
                target_location,
                ..
            }) if *target_id != 0 => (
                *target_id,
                Some(*direction),
                Some(target_location.clone()),
                ZoneNativePlayerAttackKind::Range {
                    target: target_location.clone(),
                    spell: Spell::None,
                    attack_type: 0,
                },
            ),
            WorldCommand::ClientPacket(ClientPacket::Magic {
                spell,
                direction,
                target_id,
                location,
                ..
            }) if *target_id == 0 && gateway_zone_magic_targets_ground(*spell) => (
                0,
                Some(*direction),
                Some(location.clone()),
                ZoneNativePlayerAttackKind::Magic {
                    target: location.clone(),
                    spell: *spell,
                    cast: true,
                    mp_cost: 0,
                    cooldown_ms: 0,
                },
            ),
            WorldCommand::ClientPacket(ClientPacket::Magic {
                spell,
                direction,
                target_id,
                location,
                ..
            }) if *target_id == 0 && gateway_zone_magic_targets_summon(*spell) => (
                0,
                Some(*direction),
                Some(location.clone()),
                ZoneNativePlayerAttackKind::Magic {
                    target: location.clone(),
                    spell: *spell,
                    cast: true,
                    mp_cost: 0,
                    cooldown_ms: 0,
                },
            ),
            WorldCommand::ClientPacket(ClientPacket::Magic {
                object_id: caster_object_id,
                spell,
                direction,
                target_id,
                location,
                ..
            }) if (*target_id == 0 || *target_id == *caster_object_id)
                && gateway_zone_magic_targets_self(*spell) =>
            {
                (
                    *target_id,
                    Some(*direction),
                    Some(location.clone()),
                    ZoneNativePlayerAttackKind::Magic {
                        target: location.clone(),
                        spell: *spell,
                        cast: true,
                        mp_cost: 0,
                        cooldown_ms: 0,
                    },
                )
            }
            WorldCommand::ClientPacket(ClientPacket::Magic {
                spell,
                direction,
                target_id,
                location,
                ..
            }) if *target_id != 0 => (
                *target_id,
                Some(*direction),
                Some(location.clone()),
                ZoneNativePlayerAttackKind::Magic {
                    target: location.clone(),
                    spell: *spell,
                    cast: true,
                    mp_cost: 0,
                    cooldown_ms: 0,
                },
            ),
            _ => return None,
        };
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.as_deref()?;
        let (monster, direction) = if object_id == 0 {
            (None, packet_direction)
        } else {
            let target = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .shared_entity(map_file_name, object_id)
                .or_else(|| {
                    snapshot
                        .entities
                        .iter()
                        .find(|entity| entity.object_id == object_id)
                        .cloned()
                });
            let target = target?;
            if target.kind != WorldEntityKind::Monster || target.dead {
                return None;
            }
            if requested_target.is_some_and(|point| point.x != target.x || point.y != target.y) {
                return None;
            }
            let monster = self
                .inner
                .zone_monster_spawn_snapshot(object_id)
                .or_else(|| {
                    zone_monster_spawn_from_shared_entity(
                        &target,
                        Self::zone_now_ms() / SHARED_CRYSTAL_TICK_MS,
                    )
                });
            let direction = packet_direction.or_else(|| {
                let self_entity = snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)?;
                direction_toward_points(
                    &Point {
                        x: self_entity.x,
                        y: self_entity.y,
                    },
                    &Point {
                        x: target.x,
                        y: target.y,
                    },
                )
            });
            (monster, direction)
        };
        let monster = if object_id == 0 { None } else { Some(monster?) };
        let direction = direction?;
        let (level, damage) = match &mut kind {
            ZoneNativePlayerAttackKind::Melee { .. } => {
                (0, self.inner.zone_melee_attack_damage().max(1))
            }
            ZoneNativePlayerAttackKind::Range { spell, .. } => {
                let (profile_spell, profile_level, profile_damage) =
                    self.inner.zone_range_attack_profile();
                *spell = profile_spell;
                (profile_level, profile_damage.max(1))
            }
            ZoneNativePlayerAttackKind::Magic {
                spell,
                mp_cost,
                cooldown_ms,
                ..
            } => {
                let (profile_level, profile_damage, profile_mp_cost, profile_cooldown_ms) =
                    self.inner.zone_magic_attack_profile(*spell)?;
                *mp_cost = profile_mp_cost;
                *cooldown_ms = profile_cooldown_ms;
                (profile_level, profile_damage.max(0))
            }
        };

        Some(ZoneNativePlayerAttack {
            object_id,
            direction,
            level,
            damage,
            monster,
            kind,
        })
    }

    fn execute_zone_native_player_attack(
        &mut self,
        mut attack: ZoneNativePlayerAttack,
    ) -> Vec<ServerPacket> {
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        let now_ms = Self::zone_now_ms();
        let mut packets = if let Some(monster) = attack.monster.as_ref() {
            self.dispatch_zone_player_command(
                ZoneCommand::SpawnMonster {
                    session_id: session_id.clone(),
                    monster: monster.clone(),
                    now_ms,
                },
                false,
            )
        } else {
            Vec::new()
        };
        if let ZoneNativePlayerAttackKind::Magic { spell, .. } = &attack.kind {
            if gateway_zone_magic_requires_item_consumption(*spell)
                && self.zone_native_player_attack_requires_item_consumption(&session_id, &attack)
            {
                attack.damage = attack
                    .damage
                    .saturating_add(gateway_zone_magic_item_damage_bonus(*spell));
                if !self.zone_native_player_attack_would_be_accepted(&session_id, &attack, now_ms) {
                    return Vec::new();
                }
                let Some(identity) = self.inner.active_identity() else {
                    return Vec::new();
                };
                let request_id = self.next_shared_skill_item_request_id();
                let receipt = self.account_inventory_service.commit(
                    &mut self.inner,
                    SharedAccountInventoryCommandEnvelope {
                        identity,
                        command: SharedAccountInventoryCommand::SkillItemConsume {
                            spell: *spell,
                            request_id,
                        },
                    },
                );
                if !receipt.committed {
                    return Vec::new();
                }
                packets.extend(receipt.packets);
            }
        }
        let mut accepted_magic_spend = None;
        let command = match attack.kind {
            ZoneNativePlayerAttackKind::Melee { spell, attack_type } => {
                ZoneCommand::PlayerAttackObject {
                    session_id,
                    object_id: attack.object_id,
                    direction: attack.direction,
                    spell,
                    level: attack.level,
                    attack_type,
                    damage: attack.damage,
                    now_ms,
                }
            }
            ZoneNativePlayerAttackKind::Range {
                target,
                spell,
                attack_type,
            } => ZoneCommand::PlayerRangeAttackObject {
                session_id,
                object_id: attack.object_id,
                direction: attack.direction,
                target,
                spell,
                level: attack.level,
                attack_type,
                damage: attack.damage,
                now_ms,
            },
            ZoneNativePlayerAttackKind::Magic {
                target,
                spell,
                cast,
                mp_cost,
                cooldown_ms,
            } => {
                accepted_magic_spend = cast.then_some((spell, mp_cost, cooldown_ms));
                ZoneCommand::PlayerCastMagic {
                    session_id,
                    object_id: attack.object_id,
                    spell,
                    direction: attack.direction,
                    target,
                    cast,
                    level: attack.level,
                    damage: attack.damage,
                    mp_cost,
                    cooldown_ms,
                    now_ms,
                }
            }
        };
        let dispatched = self.dispatch_zone_player_command(command, false);
        if let Some((spell, mp_cost, cooldown_ms)) = accepted_magic_spend {
            if zone_magic_launch_accepted(&dispatched, spell, attack.object_id) {
                self.inner
                    .apply_zone_player_magic_spend(spell, mp_cost, cooldown_ms);
            }
        }
        packets.extend(dispatched);
        packets
    }

    fn zone_native_player_attack_would_be_accepted(
        &self,
        session_id: &SessionId,
        attack: &ZoneNativePlayerAttack,
        now_ms: u64,
    ) -> bool {
        let ZoneNativePlayerAttackKind::Magic {
            target,
            spell,
            cast,
            mp_cost,
            cooldown_ms,
        } = &attack.kind
        else {
            return true;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .zone_manager
            .can_player_cast_magic(
                session_id,
                attack.object_id,
                *spell,
                attack.direction,
                target,
                *cast,
                attack.damage,
                *mp_cost,
                *cooldown_ms,
                now_ms,
            )
    }

    fn zone_native_player_attack_requires_item_consumption(
        &self,
        session_id: &SessionId,
        attack: &ZoneNativePlayerAttack,
    ) -> bool {
        let ZoneNativePlayerAttackKind::Magic { spell, .. } = &attack.kind else {
            return false;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .zone_manager
            .player_cast_magic_requires_item_consumption(session_id, *spell)
    }

    fn shared_harvest_target_available(&self, direction: MirDirection) -> bool {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return true;
        };
        let Some(self_entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        else {
            return true;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_harvest_allows_action(map_file_name, self_entity, direction)
    }

    fn apply_shared_action_target_snapshot(&mut self, object_id: u32) {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return;
        };
        let shared_entity = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_entity(map_file_name, object_id);
        if let Some(entity) = shared_entity {
            self.inner.apply_shared_entity_snapshot(&entity);
        }
    }

    fn apply_shared_current_map_monsters_to_local(&mut self) {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return;
        };
        let shared_entities = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_entities(map_file_name);
        for entity in shared_entities
            .iter()
            .filter(|entity| entity.kind == WorldEntityKind::Monster)
        {
            self.inner.apply_shared_entity_snapshot(entity);
        }
    }

    fn shared_npc_entity(&self, object_id: u32) -> Option<WorldEntitySnapshot> {
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.as_deref()?;
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_npc_entity(map_file_name, object_id)
    }

    fn execute_shared_npc_interact(&mut self, object_id: u32) -> Vec<ServerPacket> {
        let Some(npc) = self.shared_npc_entity(object_id) else {
            return Vec::new();
        };
        self.inner.interact_shared_npc_snapshot(&npc)
    }

    fn execute_shared_npc_call(&mut self, object_id: u32, key: &str) -> Vec<ServerPacket> {
        let Some(npc) = self.shared_npc_entity(object_id) else {
            return Vec::new();
        };
        self.inner.call_shared_npc_snapshot(&npc, key)
    }

    fn apply_shared_npc_saved_values_to_local(&mut self) {
        let values = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_npc_saved_values();
        if !values.is_empty() {
            self.inner.apply_shared_npc_saved_values(&values);
        }
    }

    #[cfg(test)]
    fn publish_shared_npc_saved_values_from_local(&mut self) {
        let values = self.inner.shared_npc_saved_values();
        if values.is_empty() {
            return;
        }
        let Some(identity) = self.inner.active_identity() else {
            return;
        };
        let receipt = self
            .npc_world_service
            .commit(SharedNpcWorldCommandEnvelope {
                identity,
                command: SharedNpcWorldCommand::SyncSavedValues(values),
            });
        if !receipt.committed {
            return;
        }
        let SharedNpcWorldCommand::SyncSavedValues(values) = receipt.command else {
            return;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .merge_shared_npc_saved_values(values);
    }

    fn apply_shared_npc_random_seed_to_local(&mut self) {
        let seed = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_npc_random_seed();
        if let Some(seed) = seed {
            self.inner.apply_shared_npc_random_seed(seed);
        }
    }

    #[cfg(test)]
    fn publish_shared_npc_random_seed_from_local(&mut self) {
        let seed = self.inner.shared_npc_random_seed();
        let Some(identity) = self.inner.active_identity() else {
            return;
        };
        let receipt = self
            .npc_world_service
            .commit(SharedNpcWorldCommandEnvelope {
                identity,
                command: SharedNpcWorldCommand::SyncRandomSeed(seed),
            });
        if !receipt.committed {
            return;
        }
        let SharedNpcWorldCommand::SyncRandomSeed(seed) = receipt.command else {
            return;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .merge_shared_npc_random_seed(seed);
    }

    #[cfg(test)]
    fn commit_shared_npc_entity_side_effect_packets(
        &mut self,
        map_file_name: String,
        packets: Vec<ServerPacket>,
    ) -> Vec<ServerPacket> {
        if packets.is_empty() {
            return Vec::new();
        }
        let Some(identity) = self.inner.active_identity() else {
            return Vec::new();
        };
        let receipt = self
            .npc_world_service
            .commit(SharedNpcWorldCommandEnvelope {
                identity,
                command: SharedNpcWorldCommand::ApplyEntitySideEffects {
                    map_file_name: map_file_name.clone(),
                    packets,
                },
            });
        if !receipt.committed {
            return Vec::new();
        }
        let SharedNpcWorldCommand::ApplyEntitySideEffects {
            map_file_name: committed_map,
            packets,
        } = receipt.command
        else {
            return Vec::new();
        };
        if committed_map != map_file_name {
            return Vec::new();
        }
        packets
    }

    fn commit_shared_npc_script_outcome(
        &mut self,
        saved_values: Vec<SharedNpcSavedValue>,
        random_seed: u64,
        entity_side_effect: Option<SharedNpcEntitySideEffect>,
    ) -> Vec<ServerPacket> {
        let Some(identity) = self.inner.active_identity() else {
            return Vec::new();
        };
        let expected_side_effect = entity_side_effect.clone();
        let receipt = self
            .npc_world_service
            .commit(SharedNpcWorldCommandEnvelope {
                identity,
                command: SharedNpcWorldCommand::ApplyScriptOutcome {
                    saved_values,
                    random_seed,
                    entity_side_effect,
                },
            });
        if !receipt.committed {
            return Vec::new();
        }
        let SharedNpcWorldCommand::ApplyScriptOutcome {
            saved_values,
            random_seed,
            entity_side_effect,
        } = receipt.command
        else {
            return Vec::new();
        };
        if !shared_npc_entity_side_effect_matches(
            expected_side_effect.as_ref(),
            entity_side_effect.as_ref(),
        ) {
            return Vec::new();
        }

        {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            zone_state.merge_shared_npc_saved_values(saved_values);
            zone_state.merge_shared_npc_random_seed(random_seed);
        }
        entity_side_effect
            .map(|side_effect| side_effect.packets)
            .unwrap_or_default()
    }

    fn execute_zone_player_packet(&mut self, packet: &ClientPacket) -> Option<Vec<ServerPacket>> {
        let session_id = self.current_zone_session_id()?;
        match packet {
            ClientPacket::Walk { direction } => {
                self.zone_move_seq = self.zone_move_seq.saturating_add(1);
                let now_ms = Self::zone_now_ms();
                let (mut packets, _, _, _, _) = self
                    .dispatch_zone_player_commands_collecting_claims(vec![
                        ZoneCommand::Walk {
                            session_id: session_id.clone(),
                            direction: *direction,
                            seq: self.zone_move_seq,
                            now_ms,
                        },
                        ZoneCommand::TickPlayerMovement { session_id, now_ms },
                    ]);
                packets.extend(self.apply_zone_current_position_map_transfer());
                Some(packets)
            }
            ClientPacket::Run { direction } => {
                self.zone_move_seq = self.zone_move_seq.saturating_add(1);
                let now_ms = Self::zone_now_ms();
                let (mut packets, _, _, _, _) = self
                    .dispatch_zone_player_commands_collecting_claims(vec![
                        ZoneCommand::Run {
                            session_id: session_id.clone(),
                            direction: *direction,
                            seq: self.zone_move_seq,
                            now_ms,
                        },
                        ZoneCommand::TickPlayerMovement { session_id, now_ms },
                    ]);
                packets.extend(self.apply_zone_current_position_map_transfer());
                Some(packets)
            }
            ClientPacket::Turn { direction } => {
                let now_ms = Self::zone_now_ms();
                let (packets, _, _, _, _) =
                    self.dispatch_zone_player_commands_collecting_claims(vec![
                        ZoneCommand::Turn {
                            session_id: session_id.clone(),
                            direction: *direction,
                            now_ms,
                        },
                        ZoneCommand::TickPlayerMovement { session_id, now_ms },
                    ]);
                Some(packets)
            }
            ClientPacket::Chat {
                message,
                linked_items,
            } => {
                // GM `@` commands and the `@LOGIN` password handshake are dispatched
                // on the personal-session path (`handle_chat_packet` ->
                // `dispatch_gm_command`), never broadcast through the shared Zone
                // (whose `chat()` silently drops `@` lines). Decline those lines here
                // so the caller falls back to `self.inner.execute(command)`. `@!`
                // announcements and `@ADDSTORAGE` stay genuine zone chat handled below.
                let trimmed = message.trim_start();
                let is_gm_command_line = trimmed.len() > 1
                    && trimmed.starts_with('@')
                    && !trimmed.starts_with("@!")
                    && !trimmed.eq_ignore_ascii_case("@ADDSTORAGE");
                if is_gm_command_line || self.inner.gm_login_pending() {
                    return None;
                }
                match self
                    .inner
                    .prepare_chat_packet_for_zone(message.clone(), linked_items.clone())
                {
                    ChatPacketPreparation::Dispatch(prepared) => {
                        Some(self.dispatch_zone_player_command(
                            ZoneCommand::Chat {
                                session_id,
                                message: prepared.message,
                                linked_items: prepared.linked_items,
                                linked_user_items: prepared.linked_user_items,
                                now_ms: Self::zone_now_ms(),
                            },
                            false,
                        ))
                    }
                    ChatPacketPreparation::Immediate(packets) => Some(packets),
                }
            }
            ClientPacket::OpenDoor { door_index } => {
                // Doors are shared world state: route to the zone, which opens
                // the door for every co-located player, broadcasts it, and
                // auto-closes it on a shared timer (Crystal `Map.Doors`).
                Some(self.dispatch_zone_player_command(
                    ZoneCommand::OpenDoor {
                        session_id,
                        door_index: *door_index,
                        now_ms: Self::zone_now_ms(),
                    },
                    false,
                ))
            }
            _ => None,
        }
    }

    fn apply_pending_shared_trade_packets(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let (deliveries, rollbacks) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            (
                zone_state.take_pending_trade_deliveries(&key),
                zone_state.take_pending_trade_rollbacks(&key),
            )
        };
        let mut packets = Vec::new();
        for offer in deliveries {
            packets.extend(self.inner.apply_shared_trade_delivery(&offer));
        }
        for offer in rollbacks {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
        }
        packets
    }

    fn apply_pending_shared_rental_packets(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let (invites, cancel_count, deliveries) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            (
                zone_state.take_pending_rental_invites(&key),
                zone_state.take_pending_rental_cancel_count(&key),
                zone_state.take_pending_rental_deliveries(&key),
            )
        };
        let mut packets = Vec::new();
        for invite in invites {
            packets.extend(
                self.inner
                    .item_rental_request(&invite.partner_name, invite.renting),
            );
        }
        for _ in 0..cancel_count {
            packets.extend(self.inner.item_rental_cancel());
        }
        for delivery in deliveries {
            packets.extend(self.inner.apply_shared_item_rental_delivery(&delivery));
        }
        packets
    }

    fn cancel_pending_shared_trade_offers(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let character_name = self
            .inner
            .active_identity()
            .map(|identity| identity.character_name)
            .unwrap_or_default();
        let own_offer = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .cancel_trade_offers_for_presence(&key, &character_name);
        own_offer
            .map(|offer| self.inner.rollback_shared_trade_offer(&offer))
            .unwrap_or_default()
    }

    fn cancel_pending_shared_rental_offers(&mut self) {
        let Some(key) = self.current_presence_key() else {
            return;
        };
        let character_name = self
            .inner
            .active_identity()
            .map(|identity| identity.character_name)
            .unwrap_or_default();
        let cancel_keys = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .cancel_rental_offers_for_presence(&key, &character_name);
        if cancel_keys.is_empty() {
            return;
        }
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        for cancel_key in cancel_keys {
            if cancel_key != key {
                zone_state.queue_rental_cancel(cancel_key);
            }
        }
    }

    fn execute_shared_item_rental_request(&mut self, partner_name: String) -> Vec<ServerPacket> {
        let packets = self.inner.item_rental_request(&partner_name, false);
        let Some(identity) = self.inner.active_identity() else {
            return packets;
        };
        let partner_key = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .player_key_by_name(&partner_name);
        if let Some(partner_key) = partner_key {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .queue_rental_invite(partner_key, identity.character_name, true);
        }
        packets
    }

    fn execute_shared_item_rental_lock_fee(&mut self) -> Vec<ServerPacket> {
        let (packets, offer) = self.inner.shared_item_rental_lock_fee();
        if let (Some(key), Some(offer)) = (self.current_presence_key(), offer) {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .rental_fee_offers
                .insert(key, offer);
        }
        packets
    }

    fn execute_shared_item_rental_lock_item(&mut self) -> Vec<ServerPacket> {
        let (packets, offer) = self.inner.shared_item_rental_lock_item();
        if let (Some(key), Some(offer)) = (self.current_presence_key(), offer) {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .rental_item_offers
                .insert(key, offer);
        }
        packets
    }

    fn execute_shared_item_rental_confirm(&mut self) -> Vec<ServerPacket> {
        let Some(self_key) = self.current_presence_key() else {
            return Vec::new();
        };

        let delivery = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            if let Some(item_offer) = zone_state.rental_item_offers.get(&self_key).cloned() {
                if let Some((fee_key, fee_offer)) =
                    zone_state.rental_fee_offer_matching_item(&item_offer)
                {
                    zone_state.rental_item_offers.remove(&self_key);
                    zone_state.rental_fee_offers.remove(&fee_key);
                    let agreement = SharedItemRentalAgreement {
                        item: item_offer,
                        fee: fee_offer,
                    };
                    zone_state
                        .pending_rental_deliveries
                        .entry(fee_key)
                        .or_default()
                        .push(SharedItemRentalDelivery::Borrower(agreement.clone()));
                    Some(SharedItemRentalDelivery::Lender(agreement))
                } else {
                    None
                }
            } else if let Some(fee_offer) = zone_state.rental_fee_offers.get(&self_key).cloned() {
                if let Some((item_key, item_offer)) =
                    zone_state.rental_item_offer_matching_fee(&fee_offer)
                {
                    zone_state.rental_fee_offers.remove(&self_key);
                    zone_state.rental_item_offers.remove(&item_key);
                    let agreement = SharedItemRentalAgreement {
                        item: item_offer,
                        fee: fee_offer,
                    };
                    zone_state
                        .pending_rental_deliveries
                        .entry(item_key)
                        .or_default()
                        .push(SharedItemRentalDelivery::Lender(agreement.clone()));
                    Some(SharedItemRentalDelivery::Borrower(agreement))
                } else {
                    None
                }
            } else {
                None
            }
        };

        delivery
            .map(|delivery| self.inner.apply_shared_item_rental_delivery(&delivery))
            .unwrap_or_default()
    }

    fn execute_shared_trade_confirm(&mut self, locked: bool) -> Vec<ServerPacket> {
        if !locked {
            let packets = self.cancel_pending_shared_trade_offers();
            if packets.is_empty() {
                return self.inner.shared_trade_cancel(true);
            }
            return packets;
        }

        let (mut packets, offer) = self.inner.shared_trade_confirm();
        let Some(offer) = offer else {
            return packets;
        };
        let Some(self_key) = self.current_presence_key() else {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
            return packets;
        };
        let self_free_bag_slots = self.inner.world_snapshot().free_bag_slots;

        let mut deliver_to_self = Vec::new();
        let mut rollback_self = None;
        {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let partner_key = zone_state.player_key_by_name(&offer.partner_name);
            if let Some(partner_key) = partner_key {
                if let Some(partner_offer) = zone_state.trade_offers.remove(&partner_key) {
                    let partner_free_bag_slots = zone_state
                        .players
                        .get(&partner_key)
                        .map(|presence| presence.free_bag_slots)
                        .unwrap_or_default();
                    if shared_trade_offer_fits(self_free_bag_slots, &partner_offer)
                        && shared_trade_offer_fits(partner_free_bag_slots, &offer)
                    {
                        zone_state
                            .pending_trade_deliveries
                            .entry(partner_key)
                            .or_default()
                            .push(offer.clone());
                        deliver_to_self.push(partner_offer);
                    } else {
                        zone_state
                            .pending_trade_rollbacks
                            .entry(partner_key)
                            .or_default()
                            .push(partner_offer);
                        rollback_self = Some(offer.clone());
                    }
                } else {
                    zone_state.trade_offers.insert(self_key, offer.clone());
                }
            } else {
                rollback_self = Some(offer.clone());
            }
        }

        if let Some(offer) = rollback_self {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
        }
        for offer in deliver_to_self {
            packets.extend(self.inner.apply_shared_trade_delivery(&offer));
        }
        packets
    }

    fn pick_up_shared_drop(&mut self, object_id: Option<u32>) -> Vec<ServerPacket> {
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        else {
            return Vec::new();
        };
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        let picker_group_members = snapshot.stage5_systems.group.members.clone();
        self.dispatch_zone_player_command(
            ZoneCommand::ClaimGroundDrop {
                session_id,
                object_id,
                target: Point {
                    x: self_entity.x,
                    y: self_entity.y,
                },
                group_members: picker_group_members,
                now_ms: Self::zone_now_ms(),
            },
            false,
        )
    }

    fn pick_up_shared_drop_with_intelligent_creature(
        &mut self,
        location: Point,
        mouse_mode: bool,
    ) -> Vec<ServerPacket> {
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        else {
            return Vec::new();
        };
        let Some(active_creature) = snapshot
            .stage5_systems
            .intelligent_creatures
            .iter()
            .find(|creature| creature.pet_mode != 0)
            .cloned()
        else {
            return Vec::new();
        };
        if active_creature.fullness < active_creature.creature_rules.minimal_fullness.max(0) {
            return Vec::new();
        }
        let target_location = if mouse_mode {
            if !active_creature.creature_rules.mouse_pickup_enabled {
                return Vec::new();
            }
            location
        } else {
            if !active_creature.creature_rules.semi_auto_pickup_enabled
                || active_creature.pet_mode != 1
            {
                return Vec::new();
            }
            Point {
                x: self_entity.x,
                y: self_entity.y,
            }
        };
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        let picker_group_members = snapshot.stage5_systems.group.members.clone();
        let (mut packets, claims, monster_kill_awards, player_damages, player_heals) = self
            .dispatch_zone_player_command_collecting_claims(
                ZoneCommand::ClaimGroundDrop {
                    session_id,
                    object_id: None,
                    target: target_location,
                    group_members: picker_group_members,
                    now_ms: Self::zone_now_ms(),
                },
                false,
            );
        self.apply_zone_player_damages(player_damages);
        self.apply_zone_player_heals(player_heals);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets, canceled_claims) =
            self.apply_shared_intelligent_creature_drop_claims(&active_creature, claims);
        remove_object_remove_packets(&mut packets, &canceled_claims);
        packets.extend(claim_packets);
        packets
    }

    fn auto_pick_up_shared_drop_with_intelligent_creature(&mut self) -> Vec<ServerPacket> {
        if self.presence_key.is_none() {
            return Vec::new();
        }
        let Some(map_file_name) = self.cached_map_file_name.as_deref() else {
            return Vec::new();
        };
        let has_shared_ground_drops = {
            let zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            zone_state.has_ground_drops_for_map(Some(map_file_name))
        };
        if has_shared_ground_drops != Some(true) {
            return Vec::new();
        }

        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        else {
            return Vec::new();
        };
        let Some(active_creature) = snapshot
            .stage5_systems
            .intelligent_creatures
            .iter()
            .find(|creature| {
                creature.pet_mode != 0
                    && creature.creature_rules.auto_pickup_enabled
                    && creature.fullness >= creature.creature_rules.minimal_fullness.max(0)
                    && creature.creature_rules.auto_pickup_range > 0
            })
            .cloned()
        else {
            return Vec::new();
        };
        let picker_location = Point {
            x: self_entity.x,
            y: self_entity.y,
        };
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        let picker_group_members = snapshot.stage5_systems.group.members.clone();
        let candidate_drops = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .map_layer(snapshot.map_file_name.as_deref())
            .map(|layer| layer.ground_drops.into_values().collect::<Vec<_>>())
            .unwrap_or_else(|| snapshot.ground_drops.clone());
        let allowed_object_ids = snapshot
            .ground_drops
            .iter()
            .chain(candidate_drops.iter())
            .filter(|drop| {
                let distance = (drop.x - picker_location.x)
                    .abs()
                    .max((drop.y - picker_location.y).abs());
                distance <= active_creature.creature_rules.auto_pickup_range
                    && intelligent_creature_allows_ground_drop(&active_creature, drop)
            })
            .map(|drop| drop.object_id)
            .collect::<BTreeSet<_>>();
        if allowed_object_ids.is_empty() {
            return Vec::new();
        }
        let (mut packets, claims, monster_kill_awards, player_damages, player_heals) = self
            .dispatch_zone_player_command_collecting_claims(
                ZoneCommand::ClaimNearestGroundDrop {
                    session_id,
                    origin: picker_location,
                    max_range: active_creature.creature_rules.auto_pickup_range,
                    allowed_object_ids,
                    group_members: picker_group_members,
                    now_ms: Self::zone_now_ms(),
                },
                false,
            );
        self.apply_zone_player_damages(player_damages);
        self.apply_zone_player_heals(player_heals);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets, canceled_claims) =
            self.apply_shared_intelligent_creature_drop_claims(&active_creature, claims);
        remove_object_remove_packets(&mut packets, &canceled_claims);
        packets.extend(claim_packets);
        packets
    }

    fn apply_shared_intelligent_creature_drop_claims(
        &mut self,
        active_creature: &mir2_protocol::ClientIntelligentCreature,
        claims: Vec<GroundDropSnapshot>,
    ) -> (Vec<ServerPacket>, BTreeSet<u32>) {
        let mut packets = Vec::new();
        let mut canceled_claims = BTreeSet::new();
        for drop in claims {
            let (claim_packets, canceled_object_id) =
                self.apply_shared_intelligent_creature_drop_claim(active_creature, drop);
            if let Some(object_id) = canceled_object_id {
                canceled_claims.insert(object_id);
            }
            packets.extend(claim_packets);
        }
        (packets, canceled_claims)
    }

    fn apply_shared_intelligent_creature_drop_claim(
        &mut self,
        active_creature: &mir2_protocol::ClientIntelligentCreature,
        drop: GroundDropSnapshot,
    ) -> (Vec<ServerPacket>, Option<u32>) {
        let Some(session_id) = self.current_zone_session_id() else {
            self.restore_zone_ground_drop_claim(drop);
            return (Vec::new(), None);
        };
        if !intelligent_creature_allows_ground_drop(active_creature, &drop) {
            let object_id = drop.object_id;
            self.restore_zone_ground_drop_claim(drop.clone());
            let packets = self.dispatch_zone_player_command(
                ZoneCommand::CancelGroundDropClaim {
                    session_id,
                    object_id,
                    now_ms: Self::zone_now_ms(),
                },
                false,
            );
            return (packets, Some(object_id));
        }
        let object_id = drop.object_id;
        let mut packets = self.inner.apply_shared_ground_drop_pickup(&drop);
        let gained = packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
            )
        });
        if gained {
            packets.insert(0, ServerPacket::IntelligentCreaturePickup { object_id });
            packets.extend(self.dispatch_zone_player_command(
                ZoneCommand::CommitGroundDropClaim {
                    session_id,
                    object_id,
                },
                false,
            ));
            (packets, None)
        } else {
            self.restore_zone_ground_drop_claim(drop);
            let packets = self.dispatch_zone_player_command(
                ZoneCommand::CancelGroundDropClaim {
                    session_id,
                    object_id,
                    now_ms: Self::zone_now_ms(),
                },
                false,
            );
            (packets, Some(object_id))
        }
    }

    fn adjacent_remote_player_name(&self) -> Option<String> {
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.as_deref()?;
        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)?;
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .remote_player_entities(Some(map_file_name), self.presence_key.as_ref())
            .into_iter()
            .find(|entity| {
                (entity.x - self_entity.x).abs() <= 1 && (entity.y - self_entity.y).abs() <= 1
            })
            .map(|entity| entity.name)
    }
}

impl Drop for SharedInProcessZoneSessionRuntime {
    fn drop(&mut self) {
        let _ = self.apply_pending_shared_trade_packets();
        let _ = self.cancel_pending_shared_trade_offers();
        self.remove_presence();
    }
}

impl WorldRuntime for SharedInProcessZoneSessionRuntime {
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.inner.on_connect()
    }

    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        let removes_presence = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::Disconnect | ClientPacket::LogOut)
        );
        let is_low_latency_zone_player_packet = matches!(
            &command,
            WorldCommand::ClientPacket(
                ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. }
            )
        );
        let is_low_latency_zone_chat_packet = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::Chat { .. })
        );
        let is_low_latency_zone_packet =
            is_low_latency_zone_player_packet || is_low_latency_zone_chat_packet;
        let is_transfer_map_command = matches!(&command, WorldCommand::TransferMap { .. });
        let is_world_tick = matches!(&command, WorldCommand::Tick);
        let skip_tail_zone_snapshot = is_low_latency_zone_packet || is_world_tick;
        let ticks_zone = matches!(
            &command,
            WorldCommand::Tick | WorldCommand::ClientPacket(ClientPacket::KeepAlive { .. })
        );
        let forwards_delayed_player_action_packets = matches!(&command, WorldCommand::Tick);
        let is_trade_cancel = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::TradeCancel)
        );
        let shared_trade_confirm = match &command {
            WorldCommand::ClientPacket(ClientPacket::TradeConfirm { locked }) => Some(*locked),
            _ => None,
        };
        let is_item_rental_lock_fee = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ItemRentalLockFee)
        );
        let is_item_rental_lock_item = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ItemRentalLockItem)
        );
        let is_item_rental_confirm = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ConfirmItemRental)
        );
        let is_item_rental_cancel = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::CancelItemRental)
        );
        let shared_rental_partner = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ItemRentalRequest)
        )
        .then(|| self.adjacent_remote_player_name())
        .flatten();
        let shared_trade_partner = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::TradeRequest)
        )
        .then(|| self.adjacent_remote_player_name())
        .flatten();
        let shared_pickup_object_id = match &command {
            WorldCommand::PickUp { object_id } => Some(Some(*object_id)),
            WorldCommand::ClientPacket(ClientPacket::PickUp) => Some(None),
            _ => None,
        };
        let shared_intelligent_creature_pickup = match &command {
            WorldCommand::ClientPacket(ClientPacket::IntelligentCreaturePickup {
                mouse_mode,
                location,
            }) => Some((*mouse_mode, location.clone())),
            _ => None,
        };
        let shared_interact_object_id = match &command {
            WorldCommand::Interact { object_id } => Some(*object_id),
            _ => None,
        };
        let shared_call_npc = match &command {
            WorldCommand::ClientPacket(ClientPacket::CallNpc { object_id, key }) => {
                Some((*object_id, key.clone()))
            }
            _ => None,
        };
        let syncs_shared_npc_world_state = matches!(
            &command,
            WorldCommand::Interact { .. }
                | WorldCommand::SelectNpcDialog { .. }
                | WorldCommand::SubmitNpcInput { .. }
                | WorldCommand::ClientPacket(
                    ClientPacket::CallNpc { .. } | ClientPacket::NpcConfirmInput { .. }
                )
        );
        let zone_native_player_attack = self.prepare_zone_native_player_attack(&command);
        let routes_zone_native_player_attack = zone_native_player_attack.is_some();
        let zone_observer_owner_id = match &command {
            _ if routes_zone_native_player_attack => None,
            WorldCommand::Attack { .. } | WorldCommand::CastSkill { .. } => {
                self.local_self_object_id()
            }
            WorldCommand::ClientPacket(
                ClientPacket::Attack { .. }
                | ClientPacket::RangeAttack { .. }
                | ClientPacket::Harvest { .. }
                | ClientPacket::Magic { .. }
                | ClientPacket::DropGold { .. }
                | ClientPacket::DropItem { .. }
                | ClientPacket::IntelligentCreaturePickup { .. },
            ) => self.local_self_object_id(),
            _ => None,
        };
        let unavailable_shared_target = match &command {
            WorldCommand::Attack { object_id } => !self.shared_action_target_available(*object_id),
            WorldCommand::ClientPacket(ClientPacket::RangeAttack { target_id, .. })
            | WorldCommand::ClientPacket(ClientPacket::Magic { target_id, .. })
                if *target_id != 0 =>
            {
                !self.shared_action_target_available(*target_id)
            }
            WorldCommand::ClientPacket(ClientPacket::Harvest { direction }) => {
                !self.shared_harvest_target_available(*direction)
            }
            _ => false,
        };
        let shared_action_target_id = match &command {
            WorldCommand::Attack { object_id } => Some(*object_id),
            WorldCommand::ClientPacket(ClientPacket::RangeAttack { target_id, .. })
            | WorldCommand::ClientPacket(ClientPacket::Magic { target_id, .. })
                if *target_id != 0 =>
            {
                Some(*target_id)
            }
            _ => None,
        };
        let needs_shared_action_snapshot = matches!(
            &command,
            WorldCommand::Attack { .. }
                | WorldCommand::CastSkill { .. }
                | WorldCommand::ClientPacket(
                    ClientPacket::Attack { .. }
                        | ClientPacket::RangeAttack { .. }
                        | ClientPacket::Harvest { .. }
                        | ClientPacket::Magic { .. }
                )
        ) && !routes_zone_native_player_attack;
        let mut tick_probe_pending_ms = 0;
        let mut tick_probe_expire_ms = 0;
        let mut tick_probe_command_ms = 0;
        let mut tick_probe_auto_pickup_ms = 0;
        let mut tick_probe_observer_ms = 0;
        let mut tick_probe_observer_npc_ms = 0;
        let mut tick_probe_observer_apply_ms = 0;
        let mut tick_probe_observer_shared_ms = 0;
        let mut tick_probe_observer_drops_ms = 0;
        let mut tick_probe_observer_quest_ms = 0;
        let mut tick_probe_observer_zone_ms = 0;
        let mut tick_probe_zone_ms = 0;
        let mut packets = if is_low_latency_zone_packet {
            Vec::new()
        } else {
            let probe_start = is_world_tick.then(Instant::now);
            let mut packets = self.apply_pending_zone_packets();
            packets.extend(self.apply_pending_shared_trade_packets());
            packets.extend(self.apply_pending_shared_rental_packets());
            if let Some(start) = probe_start {
                tick_probe_pending_ms = probe_elapsed_ms(start);
            }
            packets
        };
        if ticks_zone {
            let probe_start = is_world_tick.then(Instant::now);
            packets.extend(self.expire_shared_drops_on_current_map());
            if let Some(start) = probe_start {
                tick_probe_expire_ms = probe_elapsed_ms(start);
            }
        }
        if removes_presence {
            packets.extend(self.cancel_pending_shared_trade_offers());
            self.cancel_pending_shared_rental_offers();
            packets.extend(self.remove_presence());
        }
        if !unavailable_shared_target {
            if let Some(object_id) = shared_action_target_id {
                self.apply_shared_action_target_snapshot(object_id);
            } else if needs_shared_action_snapshot {
                self.apply_shared_current_map_monsters_to_local();
            }
        }
        if syncs_shared_npc_world_state {
            self.apply_shared_npc_saved_values_to_local();
            self.apply_shared_npc_random_seed_to_local();
        }
        let shared_npc_entity_baseline = if syncs_shared_npc_world_state {
            Some(self.inner.world_snapshot())
        } else {
            None
        };
        let command_probe_start = is_world_tick.then(Instant::now);
        let mut command_packets = if unavailable_shared_target {
            Vec::new()
        } else if let Some(locked) = shared_trade_confirm {
            self.execute_shared_trade_confirm(locked)
        } else if is_trade_cancel {
            let cancel_packets = self.cancel_pending_shared_trade_offers();
            if cancel_packets.is_empty() {
                self.inner.shared_trade_cancel(false)
            } else {
                cancel_packets
            }
        } else if is_item_rental_lock_fee {
            self.execute_shared_item_rental_lock_fee()
        } else if is_item_rental_lock_item {
            self.execute_shared_item_rental_lock_item()
        } else if is_item_rental_confirm {
            self.execute_shared_item_rental_confirm()
        } else if is_item_rental_cancel {
            self.cancel_pending_shared_rental_offers();
            self.inner.execute(command)?
        } else if let Some(partner_name) = shared_rental_partner {
            self.execute_shared_item_rental_request(partner_name)
        } else if let Some(partner_name) = shared_trade_partner {
            self.inner.trade_request(&partner_name)
        } else if let Some(attack) = zone_native_player_attack {
            self.execute_zone_native_player_attack(attack)
        } else if let WorldCommand::ClientPacket(packet) = &command {
            if let Some(zone_packets) = self.execute_zone_player_packet(packet) {
                zone_packets
            } else {
                self.inner.execute(command)?
            }
        } else {
            self.inner.execute(command)?
        };
        if let Some(start) = command_probe_start {
            tick_probe_command_ms = probe_elapsed_ms(start);
        }
        if command_packets.is_empty() {
            if let Some(object_id) = shared_interact_object_id {
                command_packets = self.execute_shared_npc_interact(object_id);
            } else if let Some((object_id, key)) = shared_call_npc {
                command_packets = self.execute_shared_npc_call(object_id, &key);
            } else if let Some((mouse_mode, location)) = shared_intelligent_creature_pickup {
                command_packets =
                    self.pick_up_shared_drop_with_intelligent_creature(location, mouse_mode);
            } else if let Some(object_id) = shared_pickup_object_id {
                command_packets = self.pick_up_shared_drop(object_id);
            }
        }
        if is_world_tick
            && !command_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::IntelligentCreaturePickup { .. }))
        {
            let probe_start = Instant::now();
            command_packets.extend(self.auto_pick_up_shared_drop_with_intelligent_creature());
            tick_probe_auto_pickup_ms = probe_elapsed_ms(probe_start);
        }
        if is_low_latency_zone_packet {
            packets.extend(command_packets);
            return Ok(packets);
        }
        let observer_probe_start = is_world_tick.then(Instant::now);
        let observer_detail_probe_start = is_world_tick.then(Instant::now);
        if let Some(before) = shared_npc_entity_baseline.as_ref() {
            let after = self.inner.world_snapshot();
            let side_effect_packets = shared_npc_entity_side_effect_packets(
                before,
                &after,
                Self::zone_now_ms() / SHARED_CRYSTAL_TICK_MS,
            );
            let entity_side_effect = (!side_effect_packets.is_empty())
                .then(|| {
                    after
                        .map_file_name
                        .map(|map_file_name| SharedNpcEntitySideEffect {
                            map_file_name,
                            packets: side_effect_packets,
                        })
                })
                .flatten();
            command_packets.extend(self.commit_shared_npc_script_outcome(
                self.inner.shared_npc_saved_values(),
                self.inner.shared_npc_random_seed(),
                entity_side_effect,
            ));
        }
        if let Some(start) = observer_detail_probe_start {
            tick_probe_observer_npc_ms = probe_elapsed_ms(start);
        }
        let observer_detail_probe_start = is_world_tick.then(Instant::now);
        self.apply_shared_entity_packets_to_current_map(&command_packets);
        if let Some(start) = observer_detail_probe_start {
            tick_probe_observer_apply_ms = probe_elapsed_ms(start);
        }
        let observer_detail_probe_start = is_world_tick.then(Instant::now);
        self.dispatch_shared_entity_observer_packets(&command_packets);
        if let Some(start) = observer_detail_probe_start {
            tick_probe_observer_shared_ms = probe_elapsed_ms(start);
        }
        let observer_detail_probe_start = is_world_tick.then(Instant::now);
        let committed_death_drop_packets =
            self.commit_shared_death_drops_to_current_map(&command_packets);
        command_packets.extend(committed_death_drop_packets);
        if let Some(start) = observer_detail_probe_start {
            tick_probe_observer_drops_ms = probe_elapsed_ms(start);
        }
        let observer_detail_probe_start = is_world_tick.then(Instant::now);
        let shared_quest_packets = self.dispatch_shared_quest_share_packets(&command_packets);
        if let Some(start) = observer_detail_probe_start {
            tick_probe_observer_quest_ms = probe_elapsed_ms(start);
        }
        let observer_detail_probe_start = is_world_tick.then(Instant::now);
        let observer_packets = if let Some(owner_id) = zone_observer_owner_id {
            self.dispatch_zone_observer_packets(owner_id, &command_packets)
        } else if is_world_tick
            && command_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::IntelligentCreaturePickup { .. }))
        {
            if let Some(owner_id) = self.local_self_object_id() {
                self.dispatch_zone_observer_packets(owner_id, &command_packets)
            } else {
                Vec::new()
            }
        } else if forwards_delayed_player_action_packets {
            if command_packets.is_empty() {
                Vec::new()
            } else if let Some(owner_id) = self.local_self_object_id() {
                let delayed_packets = delayed_player_action_packets(owner_id, &command_packets);
                self.dispatch_zone_observer_packets(owner_id, &delayed_packets)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        if let Some(start) = observer_detail_probe_start {
            tick_probe_observer_zone_ms = probe_elapsed_ms(start);
        }
        packets.extend(command_packets);
        packets.extend(shared_quest_packets);
        packets.extend(observer_packets);
        if let Some(start) = observer_probe_start {
            tick_probe_observer_ms = probe_elapsed_ms(start);
        }
        if ticks_zone {
            let probe_start = is_world_tick.then(Instant::now);
            packets.extend(self.dispatch_zone_player_command(
                ZoneCommand::Tick {
                    now_ms: Self::zone_now_ms(),
                },
                false,
            ));
            if let Some(start) = probe_start {
                tick_probe_zone_ms = probe_elapsed_ms(start);
            }
        }
        let mut packets = packets;
        if is_transfer_map_command {
            self.force_next_zone_transform_sync = true;
        }
        if !removes_presence && !skip_tail_zone_snapshot {
            packets.extend(self.sync_zone_snapshot());
        }
        if is_world_tick {
            record_shared_tick_probe(SharedTickProbeMetrics {
                count: 0,
                pending: shared_tick_probe_phase_sample(tick_probe_pending_ms),
                expire: shared_tick_probe_phase_sample(tick_probe_expire_ms),
                command: shared_tick_probe_phase_sample(tick_probe_command_ms),
                auto_pickup: shared_tick_probe_phase_sample(tick_probe_auto_pickup_ms),
                observer: shared_tick_probe_phase_sample(tick_probe_observer_ms),
                observer_detail: shared_tick_probe_observer_sample(
                    tick_probe_observer_npc_ms,
                    tick_probe_observer_apply_ms,
                    tick_probe_observer_shared_ms,
                    tick_probe_observer_drops_ms,
                    tick_probe_observer_quest_ms,
                    tick_probe_observer_zone_ms,
                ),
                zone: shared_tick_probe_phase_sample(tick_probe_zone_ms),
            });
        }
        Ok(packets)
    }

    fn world_snapshot(&self) -> WorldSnapshot {
        let mut snapshot = self.inner.world_snapshot();
        let zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        if let Some(shared_map) = zone_state.map_layer(snapshot.map_file_name.as_deref()) {
            snapshot.entities.retain(|entity| {
                matches!(
                    entity.kind,
                    WorldEntityKind::SelfPlayer | WorldEntityKind::Player
                )
            });
            snapshot.entities.extend(shared_map.entities.into_values());
            snapshot.ground_drops = shared_map.ground_drops.into_values().collect();
        }
        if let (Some(key), Some(map_file_name)) = (
            self.presence_key.as_ref(),
            snapshot.map_file_name.as_deref(),
        ) {
            if let Some(presence) = zone_state.players.get(key) {
                if presence.map_file_name == map_file_name {
                    if let Some(self_entity) = snapshot
                        .entities
                        .iter_mut()
                        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
                    {
                        self_entity.x = presence.entity.x;
                        self_entity.y = presence.entity.y;
                        self_entity.direction = presence.entity.direction;
                    }
                }
            }
        }
        let mut remote_players = zone_state.remote_player_entities(
            snapshot.map_file_name.as_deref(),
            self.presence_key.as_ref(),
        );
        snapshot.entities.append(&mut remote_players);
        snapshot.entities.sort_by_key(|entity| entity.object_id);
        snapshot
    }

    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.inner.active_identity()
    }

    fn save_active_character(&self) {
        self.inner.save_active_character();
    }

    fn refresh_active_external_mail(&mut self) -> bool {
        let changed = self.inner.refresh_active_external_mail();
        self.sync_zone_snapshot();
        changed
    }
}

pub struct RoutedZoneRuntime {
    pub zone_id: ZoneId,
    pub owner_lease: ZoneOwnerLease,
    pub owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    pub runtime: ZoneRuntimeHandle,
}

impl fmt::Debug for RoutedZoneRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutedZoneRuntime")
            .field("zone_id", &self.zone_id)
            .field("owner_lease", &self.owner_lease)
            .field("owner_lease_authority", &"ZoneOwnerLeaseAuthority")
            .field("runtime", &"WorldRuntime")
            .finish()
    }
}

#[derive(Clone)]
pub struct ZoneRegistry {
    default_zone_id: ZoneId,
    runtime_factory: SharedZoneRuntimeFactory,
    session_router: SharedSessionRouter,
    owner_lease_authority: SharedZoneOwnerLeaseAuthority,
}

impl ZoneRegistry {
    pub fn in_process() -> Self {
        Self::new(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
        )
    }

    /// In-process zone runtime + router, but with a caller-chosen owner-lease
    /// authority (e.g. the Postgres failover authority selected from the
    /// environment). Used by the production gateway bootstrap.
    pub fn in_process_with_owner_lease_authority(
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        Self::with_router_and_owner_lease_authority(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(SingleZoneSessionRouter) as SharedSessionRouter,
            owner_lease_authority,
        )
    }

    pub fn new(default_zone_id: ZoneId, runtime_factory: SharedZoneRuntimeFactory) -> Self {
        Self::with_router(
            default_zone_id,
            runtime_factory,
            Arc::new(SingleZoneSessionRouter) as SharedSessionRouter,
        )
    }

    pub fn with_router(
        default_zone_id: ZoneId,
        runtime_factory: SharedZoneRuntimeFactory,
        session_router: SharedSessionRouter,
    ) -> Self {
        Self::with_router_and_owner_lease_authority(
            default_zone_id,
            runtime_factory,
            session_router,
            Arc::new(InMemoryZoneOwnerLeaseAuthority::new()) as SharedZoneOwnerLeaseAuthority,
        )
    }

    pub fn with_router_and_owner_lease_authority(
        default_zone_id: ZoneId,
        runtime_factory: SharedZoneRuntimeFactory,
        session_router: SharedSessionRouter,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> Self {
        Self {
            default_zone_id,
            runtime_factory,
            session_router,
            owner_lease_authority,
        }
    }

    pub fn default_zone_id(&self) -> &ZoneId {
        &self.default_zone_id
    }

    pub fn open_session(&self, config: GatewayConfig) -> RoutedZoneRuntime {
        self.open_session_for(config, SessionRouteRequest::anonymous())
    }

    pub fn open_session_for(
        &self,
        config: GatewayConfig,
        route_request: SessionRouteRequest,
    ) -> RoutedZoneRuntime {
        let zone_id = self
            .session_router
            .route_session(&route_request, &self.default_zone_id);
        let owner_lease = self.owner_lease_authority.owner_lease(&zone_id);
        RoutedZoneRuntime {
            runtime: self.runtime_factory.create_runtime(config, &zone_id),
            owner_lease,
            owner_lease_authority: self.owner_lease_authority.clone(),
            zone_id,
        }
    }
}

impl Default for ZoneRegistry {
    fn default() -> Self {
        Self::in_process()
    }
}

impl fmt::Debug for ZoneRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZoneRegistry")
            .field("default_zone_id", &self.default_zone_id)
            .field("runtime_factory", &"ZoneRuntimeFactory")
            .field("session_router", &"SessionRouter")
            .field("owner_lease_authority", &"ZoneOwnerLeaseAuthority")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        delayed_player_action_packets, gateway_zone_magic_requires_item_consumption,
        gateway_zone_magic_targets_ground, gateway_zone_magic_targets_summon,
        ground_drop_spawn_packet, shared_entity_observer_packet_object_id, shared_gateway_now_ms,
        shared_npc_entity_side_effect_packets, world_entity_from_monster_info,
        zone_monster_spawn_from_shared_entity, InMemoryZoneOwnerLeaseAuthority,
        InProcessAccountInventoryService, InProcessNpcWorldService, InProcessZoneRuntimeFactory,
        MapZoneSessionRouter, PerMapSessionRouter, SessionRouteRequest, SessionRouter,
        SharedAccountInventoryCommand, SharedAccountInventoryCommandEnvelope,
        SharedAccountInventoryService, SharedAccountInventoryServiceHandle, SharedDropPickupResult,
        SharedInProcessZoneRuntimeFactory, SharedInProcessZoneSessionRuntime,
        SharedInProcessZoneState, SharedNpcEntitySideEffect, SharedNpcWorldCommand,
        SharedNpcWorldCommandEnvelope, SharedNpcWorldService, SharedNpcWorldServiceHandle,
        SharedNpcWorldTransactionReceipt, SharedSessionRouter, SharedZoneRuntimeFactory, ZoneId,
        ZoneNativePlayerAttack, ZoneNativePlayerAttackKind, ZonePresenceKey, ZoneRegistry,
    };
    use crate::{GatewayConfig, GatewaySession};
    use mir2_protocol::{
        ClientBuff, ClientIntelligentCreature, ClientPacket, IntelligentCreatureItemFilter,
        IntelligentCreatureRules, MirClass, MirDirection, MirGender, MonsterInfo, NpcInfo,
        ObjectDiedInfo, ObjectHealthInfo, ObjectMovement, ObjectPlayerInfo, ObjectRevivedInfo,
        ObjectStruckInfo, Point, ServerPacket, Spell, UserItemStat,
    };
    use mir2_simulation::{
        GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime, QuestStage,
        SharedAccountInventoryTransactionKind, SharedAccountInventoryTransactionReceipt,
        SharedNpcSavedValue, WorldCommand, WorldEntityDisposition, WorldEntityKind,
        WorldEntitySnapshot, WorldRuntime, ZoneCommand, ZoneKey, ZoneMonsterKillAward,
        ZoneOutbound,
    };
    use std::{
        collections::BTreeSet,
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    #[test]
    fn in_process_registry_routes_new_sessions_to_primary_zone() {
        let registry = ZoneRegistry::in_process();
        let mut routed = registry.open_session(GatewayConfig::default());

        assert_eq!(registry.default_zone_id(), &ZoneId::primary());
        assert_eq!(routed.zone_id, ZoneId::primary());
        assert_eq!(routed.owner_lease.zone_id(), &ZoneId::primary());
        assert_eq!(routed.owner_lease.owner_id(), "in-process:primary");
        assert_eq!(routed.owner_lease.fencing_token(), 1);
        let packets = routed
            .runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("routed runtime should execute start game");

        assert!(matches!(
            packets.first(),
            Some(ServerPacket::StartGame { .. })
        ));
    }

    #[test]
    fn in_memory_zone_owner_lease_authority_renews_ttl_before_expiry() {
        let authority = InMemoryZoneOwnerLeaseAuthority::with_lease_ttl_ms(100);
        let zone_id = ZoneId::primary();
        let lease = authority.owner_lease_at(&zone_id, 1_000);

        assert_eq!(
            authority
                .renew_owner_lease_at(&lease, 1_050)
                .expect("current owner should renew before expiry"),
            lease
        );
        assert_eq!(authority.owner_lease_at(&zone_id, 1_149), lease);

        let next = authority.owner_lease_at(&zone_id, 1_150);
        assert_eq!(next.owner_id(), "in-process:primary");
        assert_eq!(next.fencing_token(), lease.fencing_token() + 1);
    }

    #[test]
    fn in_memory_zone_owner_lease_authority_rejects_expired_renewal() {
        let authority = InMemoryZoneOwnerLeaseAuthority::with_lease_ttl_ms(100);
        let zone_id = ZoneId::primary();
        let lease = authority.owner_lease_at(&zone_id, 1_000);

        let error = authority
            .renew_owner_lease_at(&lease, 1_100)
            .expect_err("expired owner lease should not renew");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner in-process:primary fencing token 2"));
        assert!(error.contains("got owner in-process:primary fencing token 1"));
    }

    #[test]
    fn zone_registry_can_route_sessions_through_policy() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(InProcessZoneRuntimeFactory) as SharedZoneRuntimeFactory,
            Arc::new(MapZoneSessionRouter::new().with_route("0", ZoneId::new("bichon-0")))
                as SharedSessionRouter,
        );

        let routed = registry.open_session_for(
            GatewayConfig::default(),
            SessionRouteRequest {
                account_id: Some("demo".to_string()),
                character_index: Some(0),
                map_file_name: Some("0".to_string()),
            },
        );
        let default_routed = registry.open_session(GatewayConfig::default());

        assert_eq!(routed.zone_id, ZoneId::new("bichon-0"));
        assert_eq!(routed.owner_lease.zone_id(), &ZoneId::new("bichon-0"));
        assert_eq!(routed.owner_lease.owner_id(), "in-process:bichon-0");
        assert_eq!(routed.owner_lease.fencing_token(), 1);
        assert_eq!(default_routed.zone_id, ZoneId::primary());
        assert_eq!(default_routed.owner_lease.owner_id(), "in-process:primary");
    }

    #[test]
    fn per_map_router_derives_distinct_zone_per_map() {
        let router = PerMapSessionRouter::new().group("99", ZoneId::new("fields-cluster"));
        let default_zone = ZoneId::primary();
        let route = |map: Option<&str>| {
            router.route_session(
                &SessionRouteRequest {
                    account_id: None,
                    character_index: None,
                    map_file_name: map.map(str::to_string),
                },
                &default_zone,
            )
        };

        // Each distinct map derives its own zone; same map is stable.
        assert_eq!(route(Some("0")), ZoneId::new("map:0"));
        assert_eq!(route(Some("1")), ZoneId::new("map:1"));
        assert_ne!(route(Some("0")), route(Some("1")));
        assert_eq!(route(Some("0")), route(Some("0")));
        // Explicit override groups a map into a shared zone.
        assert_eq!(route(Some("99")), ZoneId::new("fields-cluster"));
        // No map yet (anonymous / pre-character-select) -> default zone.
        assert_eq!(route(None), default_zone);
    }

    #[test]
    fn per_map_routing_assigns_two_maps_to_two_zones_through_registry() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );
        let open_on_map = |map: &str, account: &str| {
            registry.open_session_for(
                GatewayConfig::default(),
                SessionRouteRequest {
                    account_id: Some(account.to_string()),
                    character_index: Some(0),
                    map_file_name: Some(map.to_string()),
                },
            )
        };

        let map0 = open_on_map("0", "a");
        let map1 = open_on_map("1", "b");
        let map0_again = open_on_map("0", "c");

        // Different maps route to different zones; same map shares one. The
        // owner lease tracks the routed zone too.
        assert_eq!(map0.zone_id, ZoneId::new("map:0"));
        assert_eq!(map1.zone_id, ZoneId::new("map:1"));
        assert_ne!(map0.zone_id, map1.zone_id);
        assert_eq!(map0_again.zone_id, ZoneId::new("map:0"));
        assert_eq!(map0.owner_lease.zone_id(), &ZoneId::new("map:0"));
    }

    // --- map=zone integration harness (oracle for the per-zone-tick + handoff steps) ---

    fn open_per_map_session(registry: &ZoneRegistry, map: &str, account: &str) -> GatewaySession {
        let routed = registry.open_session_for(
            GatewayConfig::default(),
            SessionRouteRequest {
                account_id: Some(account.to_string()),
                character_index: Some(0),
                map_file_name: Some(map.to_string()),
            },
        );
        GatewaySession::with_routed_world_runtime(routed.zone_id, routed.runtime)
    }

    fn session_sees_player(session: &GatewaySession, name: &str) -> bool {
        session
            .world_snapshot()
            .entities
            .iter()
            .any(|entity| entity.kind == WorldEntityKind::Player && entity.name == name)
    }

    #[test]
    fn map_zone_two_sessions_on_same_map_share_a_zone_and_see_each_other() {
        // Baseline that per-zone routing must preserve: two players routed to the
        // same map share one zone and are mutually visible at the gateway level.
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );

        let mut scout = open_per_map_session(&registry, "0", "scout-acct");
        start_new_character(&mut scout, "scout-acct", "Scout");
        let mut blade = open_per_map_session(&registry, "0", "blade-acct");
        start_new_character(&mut blade, "blade-acct", "Blade");

        assert_eq!(scout.zone_id(), &ZoneId::new("map:0"));
        assert_eq!(blade.zone_id(), &ZoneId::new("map:0"));
        assert!(
            session_sees_player(&blade, "Scout"),
            "blade should see scout in the shared map:0 zone"
        );
        assert!(
            session_sees_player(&scout, "Blade"),
            "scout should see blade in the shared map:0 zone"
        );
    }

    #[test]
    fn map_zone_transfer_changes_map_but_not_zone_today() {
        // Characterizes the handoff GAP: a map transfer moves the player's map but
        // leaves the session bound to its original zone. The map=zone handoff step
        // will flip the zone assertion; locking the current behavior makes that
        // change visible (and guards against an accidental partial handoff).
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );
        let mut session = open_per_map_session(&registry, "0", "wanderer");
        start_new_character(&mut session, "wanderer", "Wanderer");
        assert_eq!(session.zone_id(), &ZoneId::new("map:0"));

        // Debug crystal-transfer key relocates the player to map "0102"; mirror
        // the working transfer test (transfer + a step) to commit it.
        session.transfer_map("crystal:0:307:264");
        session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });

        let map_after = session.world_snapshot().map_file_name.clone();
        assert_eq!(
            map_after.as_deref(),
            Some("0102"),
            "transfer should move the player onto map 0102; actual: {map_after:?}"
        );
        assert_eq!(
            session.zone_id(),
            &ZoneId::new("map:0"),
            "GAP: a map transfer does NOT yet re-route the zone (map=zone handoff closes this)"
        );
    }

    #[test]
    fn shared_in_process_factory_isolates_state_by_zone_id() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(MapZoneSessionRouter::new().with_route("0", ZoneId::new("bichon-0")))
                as SharedSessionRouter,
        );
        let mut bichon = GatewaySession::with_routed_world_runtime(
            ZoneId::new("bichon-0"),
            registry
                .open_session_for(
                    GatewayConfig::default(),
                    SessionRouteRequest {
                        account_id: Some("demo".to_string()),
                        character_index: Some(0),
                        map_file_name: Some("0".to_string()),
                    },
                )
                .runtime,
        );
        let mut primary = GatewaySession::with_routed_world_runtime(
            ZoneId::primary(),
            registry.open_session(GatewayConfig::default()).runtime,
        );

        start_demo_character(&mut bichon);
        start_new_character(&mut primary, "second", "Blade");

        let bichon_snapshot = bichon.world_snapshot();
        let primary_snapshot = primary.world_snapshot();

        assert!(!bichon_snapshot
            .entities
            .iter()
            .any(|entity| { entity.kind == WorldEntityKind::Player && entity.name == "Blade" }));
        assert!(!primary_snapshot
            .entities
            .iter()
            .any(|entity| { entity.kind == WorldEntityKind::Player && entity.name == "Scout" }));
    }

    #[test]
    fn shared_zone_state_keeps_absent_viewport_entities_until_object_remove() {
        let mut state = SharedInProcessZoneState::new();
        let entity = shared_monster_entity(77);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::from([77]),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));

        state.apply_shared_entity_packets("0", &[ServerPacket::ObjectRemove { object_id: 77 }]);
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));
    }

    #[test]
    fn zone_monster_spawn_from_shared_entity_restores_crystal_neutral_ai() {
        let mut guard = shared_monster_entity(9001);
        guard.name = "Royal_Guard".to_string();
        guard.disposition = WorldEntityDisposition::Hostile;
        guard.hp = None;
        guard.max_hp = None;
        guard.sprite = None;

        let guard_spawn = zone_monster_spawn_from_shared_entity(&guard, 0)
            .expect("Royal_Guard should be convertible to a native zone spawn");
        assert_eq!(guard_spawn.ai, 6);
        assert_eq!(guard_spawn.level, 255);
        assert_eq!(guard_spawn.max_hp, 999_999);
        assert_eq!(guard_spawn.hp, 999_999);

        let mut archer = shared_monster_entity(9002);
        archer.name = "Royal_Archer".to_string();
        archer.disposition = WorldEntityDisposition::Hostile;
        archer.hp = None;
        archer.max_hp = None;
        archer.sprite = None;

        let archer_spawn = zone_monster_spawn_from_shared_entity(&archer, 0)
            .expect("Royal_Archer should be convertible to a native zone spawn");
        assert_eq!(archer_spawn.ai, 57);
        assert_eq!(archer_spawn.image, 139);
        assert_eq!(archer_spawn.level, 255);
        assert_eq!(archer_spawn.max_hp, 999_999);
    }

    #[test]
    fn zone_monster_spawn_from_shared_entity_restores_crystal_drop_templates() {
        let mut entity = shared_monster_entity(98_917);
        entity.name = "OmaFighter".to_string();
        entity.disposition = WorldEntityDisposition::Hostile;
        entity.hp = None;
        entity.max_hp = None;

        let spawn = zone_monster_spawn_from_shared_entity(&entity, 0)
            .expect("OmaFighter should be convertible to a native zone spawn");

        assert!(spawn.drops.iter().any(|drop| {
            drop.object_id == 0
                && drop.source_monster == "OmaFighter"
                && drop.ownership_remaining_ticks.is_some()
                && matches!(drop.loot, GroundDropLootSnapshot::Gold { amount } if amount > 0)
        }));
    }

    #[test]
    fn shared_npc_entity_side_effects_emit_spawn_packets_for_new_monsters() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_demo_runtime(&mut runtime);
        let mut before = runtime.inner.world_snapshot();
        before.map_file_name = Some("0".to_string());
        before.entities.clear();
        let mut after = before.clone();
        let mut monster = shared_monster_entity(98_917);
        monster.name = "Royal_Guard".to_string();
        monster.disposition = WorldEntityDisposition::Hostile;
        monster.hp = None;
        monster.max_hp = None;
        after.entities.push(monster);

        let packets = shared_npc_entity_side_effect_packets(&before, &after, 0);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.object_id == 98_917 && info.name == "Royal_Guard" && info.ai == 6
        )));
    }

    #[test]
    fn shared_npc_entity_side_effects_emit_death_packets_for_monclear() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_demo_runtime(&mut runtime);
        let mut before = runtime.inner.world_snapshot();
        before.map_file_name = Some("0".to_string());
        let alive = shared_monster_entity(77);
        let mut dead = alive.clone();
        dead.dead = true;
        dead.hp = Some(0);
        before.entities = vec![alive];
        let mut after = before.clone();
        after.entities = vec![dead];

        let packets = shared_npc_entity_side_effect_packets(&before, &after, 0);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 77 && info.percent == 0
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == 77
        )));
        assert!(packets
            .iter()
            .any(|packet| { shared_entity_observer_packet_object_id(packet) == Some(77) }));
    }

    #[test]
    fn world_entity_from_monster_info_preserves_neutral_ai_disposition() {
        let mut guard_info = shared_monster_info(9003, 0);
        guard_info.name = "Royal_Guard".to_string();
        guard_info.ai = 6;

        let guard = world_entity_from_monster_info(&guard_info);
        assert_eq!(guard.disposition, WorldEntityDisposition::Neutral);

        let mut hostile_info = shared_monster_info(9004, 0);
        hostile_info.ai = 0;

        let hostile = world_entity_from_monster_info(&hostile_info);
        assert_eq!(hostile.disposition, WorldEntityDisposition::Hostile);
    }

    #[test]
    fn shared_zone_state_keeps_absent_viewport_drops_until_object_remove_or_pickup() {
        let mut state = SharedInProcessZoneState::new();
        let picker = shared_picker_entity(9001, 330, 270);

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![shared_gold_drop(88, 330, 270, None, None)],
            BTreeSet::new(),
        );
        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::from([88]),
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .ground_drops
            .contains_key(&88));

        state.apply_shared_entity_packets("0", &[ServerPacket::ObjectRemove { object_id: 88 }]);
        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .ground_drops
            .contains_key(&88));

        state.restore_drop("0", shared_gold_drop(89, 330, 270, None, None));
        assert!(matches!(
            state.take_pickable_drop("0", Some(89), &picker, 9001, &[]),
            SharedDropPickupResult::Picked(_)
        ));
        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .ground_drops
            .contains_key(&89));
    }

    #[test]
    fn shared_zone_state_treats_intelligent_creature_pickup_as_drop_remove() {
        let mut state = SharedInProcessZoneState::new();

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![shared_gold_drop(88, 330, 270, None, None)],
            BTreeSet::new(),
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .ground_drops
            .contains_key(&88));

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::IntelligentCreaturePickup { object_id: 88 }],
        );
        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![shared_gold_drop(88, 330, 270, None, None)],
            BTreeSet::new(),
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        assert!(!map.ground_drops.contains_key(&88));
        assert!(map.removed_drop_ids.contains(&88));
    }

    #[test]
    fn shared_zone_state_records_object_monster_spawn_packet() {
        let mut state = SharedInProcessZoneState::new();

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectMonster {
                info: MonsterInfo {
                    object_id: 501,
                    name: "Shinsu".to_string(),
                    name_colour_argb: -1,
                    location: Point { x: 331, y: 270 },
                    image: 33,
                    direction: MirDirection::Down,
                    effect: 0,
                    ai: 6,
                    light: 0,
                    dead: false,
                    skeleton: false,
                    poison: 0,
                    hidden: false,
                    shock_time: 0,
                    binding_shot_center: false,
                    extra: false,
                    extra_byte: 0,
                    master_object_id: 101,
                    rarity: 0,
                    buffs: vec![7],
                },
            }],
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        let entity = map
            .entities
            .get(&501)
            .expect("monster spawn packet should create shared entity");
        assert_eq!(entity.name, "Shinsu");
        assert_eq!(entity.kind, WorldEntityKind::Monster);
        assert_eq!((entity.x, entity.y), (331, 270));
        assert_eq!(entity.direction, MirDirection::Down);
        // ai 6 is a Crystal Neutral AI (see monster_disposition_for_ai /
        // world_entity_disposition_for_monster_ai); the shared-zone packet path
        // must record it as Neutral, matching world_entity_from_monster_info.
        assert_eq!(entity.disposition, WorldEntityDisposition::Neutral);
        assert_eq!(
            entity
                .sprite
                .as_ref()
                .map(|sprite| sprite.body_library.as_str()),
            Some("Monster/033")
        );
    }

    #[test]
    fn shared_zone_state_applies_dead_marker_to_late_object_monster_packet() {
        let mut state = SharedInProcessZoneState::new();

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 501,
                    location: Point { x: 335, y: 275 },
                    direction: MirDirection::Left,
                    kind: 0,
                },
            }],
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectMonster {
                info: MonsterInfo {
                    object_id: 501,
                    name: "Shinsu".to_string(),
                    name_colour_argb: -1,
                    location: Point { x: 331, y: 270 },
                    image: 33,
                    direction: MirDirection::Down,
                    effect: 0,
                    ai: 6,
                    light: 0,
                    dead: false,
                    skeleton: false,
                    poison: 0,
                    hidden: false,
                    shock_time: 0,
                    binding_shot_center: false,
                    extra: false,
                    extra_byte: 0,
                    master_object_id: 101,
                    rarity: 0,
                    buffs: Vec::new(),
                },
            }],
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        let entity = map
            .entities
            .get(&501)
            .expect("late monster packet should still materialize a dead entity");
        assert!(entity.dead);
        assert_eq!(entity.hp, Some(0));
        assert_eq!((entity.x, entity.y), (335, 275));
        assert_eq!(entity.direction, MirDirection::Left);
    }

    #[test]
    fn shared_zone_state_records_object_hero_and_npc_spawn_packets() {
        let mut state = SharedInProcessZoneState::new();

        state.apply_shared_entity_packets(
            "0",
            &[
                ServerPacket::ObjectHero {
                    info: ObjectPlayerInfo {
                        object_id: 601,
                        name: "ScoutHero".to_string(),
                        guild_name: String::new(),
                        guild_rank_name: String::new(),
                        name_colour_argb: -1,
                        class: MirClass::Taoist,
                        gender: MirGender::Female,
                        level: 7,
                        location: Point { x: 331, y: 271 },
                        direction: MirDirection::Left,
                        hair: 0,
                        light: 0,
                        weapon: -1,
                        weapon_effect: 0,
                        armour: -1,
                        poison: 0,
                        dead: false,
                        hidden: false,
                        effect: 0,
                        wing_effect: 0,
                        extra: false,
                        mount_type: -1,
                        riding_mount: false,
                        fishing: false,
                        transform_type: 0,
                        element_orb_effect: 0,
                        element_orb_level: 0,
                        element_orb_max: 0,
                        buffs: Vec::new(),
                        level_effects: 0,
                    },
                    owner_name: "Scout".to_string(),
                },
                ServerPacket::ObjectNpc {
                    info: NpcInfo {
                        object_id: 701,
                        name: "Village Guide".to_string(),
                        name_colour_argb: -1,
                        image: 12,
                        colour_argb: -1,
                        location: Point { x: 329, y: 270 },
                        direction: MirDirection::Down,
                        quest_ids: vec![1, 2],
                    },
                },
            ],
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        let hero = map
            .entities
            .get(&601)
            .expect("hero spawn packet should create shared entity");
        assert_eq!(hero.kind, WorldEntityKind::Player);
        assert_eq!(hero.name, "ScoutHero");
        assert_eq!(hero.owner_name.as_deref(), Some("Scout"));
        assert_eq!(hero.class, Some(MirClass::Taoist));
        assert_eq!(hero.gender, Some(MirGender::Female));
        assert_eq!((hero.x, hero.y), (331, 271));

        let npc = map
            .entities
            .get(&701)
            .expect("npc spawn packet should create shared entity");
        assert_eq!(npc.kind, WorldEntityKind::Npc);
        assert_eq!(npc.name, "Village Guide");
        assert_eq!(npc.quest_ids, vec![1, 2]);
        assert_eq!(npc.disposition, WorldEntityDisposition::Neutral);
        assert_eq!(
            npc.sprite
                .as_ref()
                .map(|sprite| sprite.body_library.as_str()),
            Some("NPC/12")
        );
    }

    #[test]
    fn shared_zone_state_records_summon_owner_from_monster_master() {
        let mut state = SharedInProcessZoneState::new();
        let owner_key = ZonePresenceKey {
            account_id: "owner".to_string(),
            character_index: 0,
        };
        let owner_zone_object_id = state.upsert_player(
            owner_key,
            "Owner",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectMonster {
                info: shared_monster_info(501, owner_zone_object_id),
            }],
        );

        let entity = state
            .shared_entity("0", 501)
            .expect("summoned monster should be tracked");
        assert_eq!(entity.owner_name.as_deref(), Some("Owner"));
    }

    #[test]
    fn shared_zone_state_preserves_owned_generated_entity_owner_during_snapshot_merge() {
        let mut state = SharedInProcessZoneState::new();
        let owner_key = ZonePresenceKey {
            account_id: "owner".to_string(),
            character_index: 0,
        };
        let owner_zone_object_id = state.upsert_player(
            owner_key,
            "Owner",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectMonster {
                info: shared_monster_info(501, owner_zone_object_id),
            }],
        );
        let stale_snapshot = shared_monster_entity(501);
        assert!(stale_snapshot.owner_name.is_none());

        state.sync_map_layer(
            "0".to_string(),
            vec![stale_snapshot],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let entity = state
            .shared_entity("0", 501)
            .expect("summoned monster should remain tracked");
        assert_eq!(entity.owner_name.as_deref(), Some("Owner"));
    }

    #[test]
    fn shared_zone_state_removes_owned_generated_entities_on_player_leave() {
        let mut state = SharedInProcessZoneState::new();
        let owner_key = ZonePresenceKey {
            account_id: "owner".to_string(),
            character_index: 0,
        };
        let observer_key = ZonePresenceKey {
            account_id: "observer".to_string(),
            character_index: 0,
        };
        state.upsert_player(
            owner_key.clone(),
            "Owner",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        state.upsert_player(
            observer_key.clone(),
            "Observer",
            "0".to_string(),
            shared_picker_entity(102, 331, 270),
            80,
        );
        let mut summon = shared_monster_entity(501);
        summon.owner_name = Some("Owner".to_string());
        let mut hero = shared_picker_entity(601, 332, 270);
        hero.kind = WorldEntityKind::Player;
        hero.owner_name = Some("Owner".to_string());
        state.sync_map_layer(
            "0".to_string(),
            vec![summon, hero],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        state.remove_player(&owner_key);

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        assert!(!map.entities.contains_key(&501));
        assert!(!map.entities.contains_key(&601));
        assert!(map.removed_entity_ids.contains(&501));
        assert!(map.removed_entity_ids.contains(&601));
        let observer_packets = state.take_pending_zone_packets(&observer_key);
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 501
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 601
        )));
    }

    #[test]
    fn shared_zone_state_coalesces_pending_movement_by_object() {
        let mut state = SharedInProcessZoneState::new();
        let observer_key = ZonePresenceKey {
            account_id: "observer".to_string(),
            character_index: 0,
        };

        state.queue_zone_packets(
            observer_key.clone(),
            vec![
                ServerPacket::ObjectWalk {
                    movement: ObjectMovement {
                        object_id: 77,
                        position: Point { x: 330, y: 270 },
                        direction: MirDirection::Right,
                    },
                },
                ServerPacket::ObjectRun {
                    movement: ObjectMovement {
                        object_id: 77,
                        position: Point { x: 332, y: 270 },
                        direction: MirDirection::Right,
                    },
                },
                ServerPacket::ObjectWalk {
                    movement: ObjectMovement {
                        object_id: 88,
                        position: Point { x: 331, y: 270 },
                        direction: MirDirection::Left,
                    },
                },
            ],
        );
        let packets = state.take_pending_zone_packets(&observer_key);

        assert_eq!(packets.len(), 2);
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRun { movement }
                if movement.object_id == 77 && movement.position.x == 332
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == 88 && movement.position.x == 331
        )));
    }

    #[test]
    fn shared_zone_state_applies_object_movement_packets_to_shared_entities() {
        let mut state = SharedInProcessZoneState::new();

        state.sync_map_layer(
            "0".to_string(),
            vec![shared_monster_entity(77)],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[
                ServerPacket::ObjectWalk {
                    movement: ObjectMovement {
                        object_id: 77,
                        position: Point { x: 330, y: 270 },
                        direction: MirDirection::Right,
                    },
                },
                ServerPacket::ObjectRun {
                    movement: ObjectMovement {
                        object_id: 77,
                        position: Point { x: 332, y: 270 },
                        direction: MirDirection::Right,
                    },
                },
                ServerPacket::ObjectTurn {
                    movement: ObjectMovement {
                        object_id: 77,
                        position: Point { x: 332, y: 270 },
                        direction: MirDirection::Up,
                    },
                },
            ],
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("shared monster should remain present");
        assert_eq!((entity.x, entity.y), (332, 270));
        assert_eq!(entity.direction, MirDirection::Up);
    }

    #[test]
    fn shared_zone_state_applies_object_transform_packets_to_shared_entities() {
        let mut state = SharedInProcessZoneState::new();

        state.sync_map_layer(
            "0".to_string(),
            vec![shared_monster_entity(77)],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[
                ServerPacket::ObjectPushed {
                    object_id: 77,
                    location: Point { x: 331, y: 270 },
                    direction: MirDirection::Right,
                },
                ServerPacket::ObjectBackStep {
                    movement: ObjectMovement {
                        object_id: 77,
                        position: Point { x: 330, y: 269 },
                        direction: MirDirection::Up,
                    },
                    distance: 1,
                },
            ],
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("shared monster should remain present");
        assert_eq!((entity.x, entity.y), (330, 269));
        assert_eq!(entity.direction, MirDirection::Up);
    }

    #[test]
    fn shared_zone_state_ignores_object_transform_packets_for_dead_entities() {
        let mut state = SharedInProcessZoneState::new();

        state.sync_map_layer(
            "0".to_string(),
            vec![shared_monster_entity(77)],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 77,
                    location: Point { x: 329, y: 269 },
                    direction: MirDirection::Down,
                    kind: 0,
                },
            }],
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectPushed {
                object_id: 77,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Right,
            }],
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("dead shared monster should remain present");
        assert_eq!((entity.x, entity.y), (329, 269));
        assert_eq!(entity.direction, MirDirection::Down);
        assert!(entity.dead);
    }

    #[test]
    fn shared_zone_state_save_transform_updates_presence_before_pending_drain() {
        let mut state = SharedInProcessZoneState::new();
        let current_key = ZonePresenceKey {
            account_id: "current".to_string(),
            character_index: 0,
        };
        let remote_key = ZonePresenceKey {
            account_id: "remote".to_string(),
            character_index: 0,
        };
        let current_session_id = SharedInProcessZoneState::zone_session_id_for_key(&current_key);
        let remote_session_id = SharedInProcessZoneState::zone_session_id_for_key(&remote_key);
        let current_position = Point { x: 331, y: 271 };
        let remote_position = Point { x: 337, y: 275 };

        state.upsert_player(
            current_key.clone(),
            "Current",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        state.upsert_player(
            remote_key.clone(),
            "Remote",
            "0".to_string(),
            shared_picker_entity(102, 336, 274),
            80,
        );
        state
            .zone_session_keys
            .insert(current_session_id.clone(), current_key.clone());
        state
            .zone_session_keys
            .insert(remote_session_id.clone(), remote_key.clone());

        let (_, current_transform, _, _, _, _, _) = state.dispatch_zone_outbounds(
            vec![
                ZoneOutbound::SaveTransform {
                    session_id: current_session_id,
                    position: current_position.clone(),
                    direction: MirDirection::Up,
                },
                ZoneOutbound::SaveTransform {
                    session_id: remote_session_id,
                    position: remote_position.clone(),
                    direction: MirDirection::Right,
                },
            ],
            Some(&current_key),
        );

        assert_eq!(
            current_transform,
            Some((current_position.clone(), MirDirection::Up))
        );
        assert_eq!(state.take_pending_zone_transform(&current_key), None);
        assert_eq!(
            state.take_pending_zone_transform(&remote_key),
            Some((remote_position.clone(), MirDirection::Right))
        );

        let current_presence = state
            .players
            .get(&current_key)
            .expect("current presence should remain");
        assert_eq!(
            (
                current_presence.entity.x,
                current_presence.entity.y,
                current_presence.entity.direction,
            ),
            (current_position.x, current_position.y, MirDirection::Up)
        );
        let remote_presence = state
            .players
            .get(&remote_key)
            .expect("remote presence should remain");
        assert_eq!(
            (
                remote_presence.entity.x,
                remote_presence.entity.y,
                remote_presence.entity.direction,
            ),
            (remote_position.x, remote_position.y, MirDirection::Right)
        );
    }

    #[test]
    fn shared_zone_state_dispatches_current_and_pending_monster_kill_awards() {
        let mut state = SharedInProcessZoneState::new();
        let current_key = ZonePresenceKey {
            account_id: "current".to_string(),
            character_index: 0,
        };
        let remote_key = ZonePresenceKey {
            account_id: "remote".to_string(),
            character_index: 0,
        };
        let current_session_id = SharedInProcessZoneState::zone_session_id_for_key(&current_key);
        let remote_session_id = SharedInProcessZoneState::zone_session_id_for_key(&remote_key);
        state
            .zone_session_keys
            .insert(current_session_id.clone(), current_key.clone());
        state
            .zone_session_keys
            .insert(remote_session_id.clone(), remote_key.clone());

        let award = ZoneMonsterKillAward {
            monster_object_id: 9100,
            monster_name: "Field Wasp".to_string(),
            experience: 6,
            drops: Vec::new(),
        };
        let (_, _, _, _, current_awards, _, _) = state.dispatch_zone_outbounds(
            vec![
                ZoneOutbound::MonsterKillAward {
                    session_id: current_session_id,
                    award: award.clone(),
                },
                ZoneOutbound::MonsterKillAward {
                    session_id: remote_session_id,
                    award: award.clone(),
                },
            ],
            Some(&current_key),
        );

        assert_eq!(current_awards, vec![award.clone()]);
        assert_eq!(
            state.take_pending_zone_monster_kill_awards(&remote_key),
            vec![award]
        );
    }

    #[test]
    fn shared_in_process_runtime_emits_gain_experience_after_kill_award_commit() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "zone-kill-award", "Blade");
        let before_experience = runtime.inner.world_snapshot().player_experience;

        let packets = runtime.apply_zone_monster_kill_awards(vec![ZoneMonsterKillAward {
            monster_object_id: 9100,
            monster_name: "Field Wasp".to_string(),
            experience: 6,
            drops: Vec::new(),
        }]);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainExperience { amount } if *amount == 6
        )));
        assert_eq!(
            runtime.inner.world_snapshot().player_experience,
            before_experience + 6
        );
    }

    #[test]
    fn shared_in_process_runtime_uses_account_inventory_receipts_for_zone_rewards() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "zone-account-inventory", "Blade");
        let before_experience = runtime.inner.world_snapshot().player_experience;

        let award_receipt =
            runtime
                .inner
                .commit_shared_monster_kill_award_transaction(9100, "Field Wasp", 6);
        assert_eq!(
            award_receipt.kind,
            SharedAccountInventoryTransactionKind::MonsterKillAward
        );
        assert!(award_receipt.committed);
        assert!(award_receipt.packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainExperience { amount } if *amount == 6
        )));
        assert_eq!(
            runtime.inner.world_snapshot().player_experience,
            before_experience + 6
        );

        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let gold_drop = GroundDropSnapshot {
            object_id: 9101,
            name: "Receipt Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: self_entity.x,
            y: self_entity.y,
            quantity: 1,
            source_monster: "receipt-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 25 },
        };
        let pickup_receipt = runtime
            .inner
            .commit_shared_ground_drop_pickup_transaction(&gold_drop);
        assert_eq!(
            pickup_receipt.kind,
            SharedAccountInventoryTransactionKind::GroundDropPickup
        );
        assert!(pickup_receipt.committed);
        assert!(pickup_receipt.packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedGold { gold } if *gold == 25
        )));

        let rejected_drop = GroundDropSnapshot {
            object_id: 9102,
            name: "Overflow Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: self_entity.x,
            y: self_entity.y,
            quantity: u32::MAX,
            source_monster: "receipt-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: u32::MAX },
        };
        let rejected_receipt = runtime
            .inner
            .commit_shared_ground_drop_pickup_transaction(&rejected_drop);
        assert_eq!(
            rejected_receipt.kind,
            SharedAccountInventoryTransactionKind::GroundDropPickup
        );
        assert!(!rejected_receipt.committed);
        assert!(rejected_receipt
            .packets
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
    }

    #[test]
    fn in_process_account_inventory_service_rejects_identity_mismatch() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "inventory-identity-owner", "Blade");
        let mut wrong_identity = runtime
            .inner
            .active_identity()
            .expect("started runtime should have an active identity");
        wrong_identity.account_id = "other-account".to_string();
        let before_experience = runtime.inner.world_snapshot().player_experience;

        let service = InProcessAccountInventoryService::new();
        let receipt = service.commit(
            &mut runtime.inner,
            SharedAccountInventoryCommandEnvelope {
                identity: wrong_identity,
                command: SharedAccountInventoryCommand::MonsterKillAward(ZoneMonsterKillAward {
                    monster_object_id: 9100,
                    monster_name: "Field Wasp".to_string(),
                    experience: 6,
                    drops: Vec::new(),
                }),
            },
        );

        assert_eq!(
            receipt.kind,
            SharedAccountInventoryTransactionKind::MonsterKillAward
        );
        assert!(!receipt.committed);
        assert!(receipt.packets.is_empty());
        assert_eq!(
            runtime.inner.world_snapshot().player_experience,
            before_experience
        );
    }

    #[test]
    fn in_process_account_inventory_service_handles_skill_item_consumption_command() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "inventory-skill-item-owner", "Blade");
        let identity = runtime
            .inner
            .active_identity()
            .expect("started runtime should have an active identity");

        let service = InProcessAccountInventoryService::new();
        let receipt = service.commit(
            &mut runtime.inner,
            SharedAccountInventoryCommandEnvelope {
                identity,
                command: SharedAccountInventoryCommand::SkillItemConsume {
                    spell: Spell::PoisonCloud,
                    request_id: 1,
                },
            },
        );

        assert_eq!(
            receipt.kind,
            SharedAccountInventoryTransactionKind::SkillItemConsumption
        );
        assert!(!receipt.committed);
        assert!(receipt.packets.is_empty());
    }

    #[test]
    fn in_process_account_inventory_service_deduplicates_committed_zone_rewards() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "inventory-idempotent-owner", "Blade");
        let identity = runtime
            .inner
            .active_identity()
            .expect("started runtime should have an active identity");
        let service = InProcessAccountInventoryService::new();
        let before_experience = runtime.inner.world_snapshot().player_experience;

        let award_envelope = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::MonsterKillAward(ZoneMonsterKillAward {
                monster_object_id: 9100,
                monster_name: "Field Wasp".to_string(),
                experience: 6,
                drops: Vec::new(),
            }),
        };
        let first_award = service.commit(&mut runtime.inner, award_envelope.clone());
        let after_award_experience = runtime.inner.world_snapshot().player_experience;
        let retry_award = service.commit(&mut runtime.inner, award_envelope);

        assert!(first_award.committed);
        assert_eq!(retry_award, first_award);
        assert_eq!(after_award_experience, before_experience + 6);
        assert_eq!(
            runtime.inner.world_snapshot().player_experience,
            after_award_experience
        );

        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let before_gold = runtime.inner.world_snapshot().gold;
        let pickup_envelope = SharedAccountInventoryCommandEnvelope {
            identity,
            command: SharedAccountInventoryCommand::GroundDropPickup(GroundDropSnapshot {
                object_id: 9101,
                name: "Idempotent Gold".to_string(),
                name_colour_argb: -1,
                icon: 0,
                x: self_entity.x,
                y: self_entity.y,
                quantity: 1,
                source_monster: "idempotent-test".to_string(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 25 },
            }),
        };
        let first_pickup = service.commit(&mut runtime.inner, pickup_envelope.clone());
        let after_pickup_gold = runtime.inner.world_snapshot().gold;
        let retry_pickup = service.commit(&mut runtime.inner, pickup_envelope);

        assert!(first_pickup.committed);
        assert_eq!(retry_pickup, first_pickup);
        assert_eq!(after_pickup_gold, before_gold + 25);
        assert_eq!(runtime.inner.world_snapshot().gold, after_pickup_gold);
    }

    #[test]
    fn shared_account_inventory_skill_item_key_includes_request_id() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "inventory-skill-idempotency-key", "Sage");
        let identity = runtime
            .inner
            .active_identity()
            .expect("started runtime should have an active identity");
        let first = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonSkeleton,
                request_id: 42,
            },
        };
        let same = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonSkeleton,
                request_id: 42,
            },
        };
        let distinct_request = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonSkeleton,
                request_id: 43,
            },
        };
        let distinct_spell = SharedAccountInventoryCommandEnvelope {
            identity,
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonShinsu,
                request_id: 42,
            },
        };

        assert_eq!(first.idempotency_key(), same.idempotency_key());
        assert_ne!(first.idempotency_key(), distinct_request.idempotency_key());
        assert_ne!(first.idempotency_key(), distinct_spell.idempotency_key());
    }

    #[derive(Debug)]
    struct RecordingAccountInventoryService {
        commands: Arc<Mutex<Vec<SharedAccountInventoryCommandEnvelope>>>,
        ground_drop_calls: Arc<Mutex<usize>>,
        monster_award_calls: Arc<Mutex<usize>>,
        skill_item_calls: Arc<Mutex<usize>>,
        ground_drop_committed: bool,
        monster_award_committed: bool,
        skill_item_committed: bool,
    }

    impl SharedAccountInventoryService for RecordingAccountInventoryService {
        fn commit(
            &self,
            _runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            let command = envelope.command.clone();
            self.commands
                .lock()
                .expect("account inventory commands should lock")
                .push(envelope);
            match command {
                SharedAccountInventoryCommand::GroundDropPickup(_) => {
                    *self
                        .ground_drop_calls
                        .lock()
                        .expect("ground drop calls should lock") += 1;
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
                        committed: self.ground_drop_committed,
                        packets: self
                            .ground_drop_committed
                            .then(|| ServerPacket::GainedGold { gold: 25 })
                            .into_iter()
                            .collect(),
                    }
                }
                SharedAccountInventoryCommand::MonsterKillAward(_) => {
                    *self
                        .monster_award_calls
                        .lock()
                        .expect("monster award calls should lock") += 1;
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::MonsterKillAward,
                        committed: self.monster_award_committed,
                        packets: self
                            .monster_award_committed
                            .then(|| ServerPacket::GainExperience { amount: 777 })
                            .into_iter()
                            .collect(),
                    }
                }
                SharedAccountInventoryCommand::SkillItemConsume { .. } => {
                    *self
                        .skill_item_calls
                        .lock()
                        .expect("skill item calls should lock") += 1;
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::SkillItemConsumption,
                        committed: self.skill_item_committed,
                        packets: self
                            .skill_item_committed
                            .then(|| ServerPacket::DeleteItem {
                                unique_id: 9,
                                count: 5,
                            })
                            .into_iter()
                            .collect(),
                    }
                }
            }
        }
    }

    #[test]
    fn shared_in_process_runtime_uses_account_inventory_service_boundary() {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let ground_drop_calls = Arc::new(Mutex::new(0));
        let monster_award_calls = Arc::new(Mutex::new(0));
        let skill_item_calls = Arc::new(Mutex::new(0));
        let service = Arc::new(RecordingAccountInventoryService {
            commands: commands.clone(),
            ground_drop_calls: ground_drop_calls.clone(),
            monster_award_calls: monster_award_calls.clone(),
            skill_item_calls: skill_item_calls.clone(),
            ground_drop_committed: false,
            monster_award_committed: true,
            skill_item_committed: true,
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "zone-account-inventory-service", "Blade");
        let before_experience = runtime.inner.world_snapshot().player_experience;

        let award_packets = runtime.apply_zone_monster_kill_awards(vec![ZoneMonsterKillAward {
            monster_object_id: 9100,
            monster_name: "Field Wasp".to_string(),
            experience: 6,
            drops: Vec::new(),
        }]);
        assert_eq!(
            *monster_award_calls
                .lock()
                .expect("monster award calls should lock"),
            1
        );
        assert!(commands
            .lock()
            .expect("account inventory commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-account-inventory-service"
                    && envelope.identity.character_name == "Blade"
                    && matches!(
                        &envelope.command,
                        SharedAccountInventoryCommand::MonsterKillAward(award)
                            if award.monster_object_id == 9100
                                && award.monster_name == "Field Wasp"
                                && award.experience == 6
                    )
            }));
        assert!(award_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainExperience { amount } if *amount == 777
        )));
        assert_eq!(
            runtime.inner.world_snapshot().player_experience,
            before_experience
        );

        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let gold_drop = GroundDropSnapshot {
            object_id: 9101,
            name: "Actor Boundary Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: self_entity.x,
            y: self_entity.y,
            quantity: 1,
            source_monster: "actor-boundary-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 25 },
        };
        let (pickup_packets, canceled_claims) =
            runtime.apply_zone_ground_drop_claims(vec![gold_drop]);
        assert_eq!(
            *ground_drop_calls
                .lock()
                .expect("ground drop calls should lock"),
            1
        );
        assert!(commands
            .lock()
            .expect("account inventory commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-account-inventory-service"
                    && envelope.identity.character_name == "Blade"
                    && matches!(
                        &envelope.command,
                        SharedAccountInventoryCommand::GroundDropPickup(drop)
                            if drop.object_id == 9101 && drop.name == "Actor Boundary Gold"
                    )
            }));
        assert!(canceled_claims.contains(&9101));
        assert!(pickup_packets
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
    }

    #[test]
    fn shared_in_process_runtime_prechecks_item_skill_before_consuming_items() {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let skill_item_calls = Arc::new(Mutex::new(0));
        let service = Arc::new(RecordingAccountInventoryService {
            commands: commands.clone(),
            ground_drop_calls: Arc::new(Mutex::new(0)),
            monster_award_calls: Arc::new(Mutex::new(0)),
            skill_item_calls: skill_item_calls.clone(),
            ground_drop_committed: true,
            monster_award_committed: true,
            skill_item_committed: true,
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "zone-skill-item-precheck", "Blade");

        assert!(gateway_zone_magic_targets_ground(Spell::PoisonCloud));
        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let target = Point {
            x: self_entity.x + 1,
            y: self_entity.y,
        };
        let attack = ZoneNativePlayerAttack {
            object_id: 0,
            direction: MirDirection::Right,
            level: 3,
            damage: 5,
            monster: None,
            kind: ZoneNativePlayerAttackKind::Magic {
                target: target.clone(),
                spell: Spell::PoisonCloud,
                cast: true,
                mp_cost: 0,
                cooldown_ms: 10_000,
            },
        };

        let first_cast = runtime.execute_zone_native_player_attack(attack.clone());
        assert_eq!(
            *skill_item_calls
                .lock()
                .expect("skill item calls should lock"),
            1
        );
        assert!(commands
            .lock()
            .expect("account inventory commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-skill-item-precheck"
                    && matches!(
                        &envelope.command,
                        SharedAccountInventoryCommand::SkillItemConsume {
                            spell: Spell::PoisonCloud,
                            request_id: 1,
                        }
                    )
            }));
        assert!(first_cast
            .iter()
            .any(|packet| matches!(packet, ServerPacket::DeleteItem { count: 5, .. })));
        assert!(first_cast.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::PoisonCloud,
                target_id: 0,
                target: packet_target,
                ..
            } if packet_target == &target
        )));

        let early_retry = runtime.execute_zone_native_player_attack(attack);
        assert!(early_retry.is_empty());
        assert_eq!(
            *skill_item_calls
                .lock()
                .expect("skill item calls should lock"),
            1,
            "Zone rejection must happen before item-consumption commit"
        );
    }

    #[test]
    fn shared_in_process_runtime_routes_summon_magic_through_zone_item_boundary() {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let skill_item_calls = Arc::new(Mutex::new(0));
        let service = Arc::new(RecordingAccountInventoryService {
            commands: commands.clone(),
            ground_drop_calls: Arc::new(Mutex::new(0)),
            monster_award_calls: Arc::new(Mutex::new(0)),
            skill_item_calls: skill_item_calls.clone(),
            ground_drop_committed: true,
            monster_award_committed: true,
            skill_item_committed: true,
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "zone-summon-item-boundary", "Sage");

        assert!(gateway_zone_magic_targets_summon(Spell::SummonSkeleton));
        assert!(gateway_zone_magic_targets_summon(Spell::SummonShinsu));
        assert!(gateway_zone_magic_targets_summon(Spell::SummonVampire));
        assert!(gateway_zone_magic_targets_summon(Spell::SummonToad));
        assert!(gateway_zone_magic_targets_summon(Spell::SummonSnakes));
        assert!(gateway_zone_magic_targets_summon(Spell::Stonetrap));
        assert!(gateway_zone_magic_requires_item_consumption(
            Spell::SummonSkeleton
        ));
        assert!(gateway_zone_magic_requires_item_consumption(
            Spell::SummonShinsu
        ));
        assert!(!gateway_zone_magic_requires_item_consumption(
            Spell::SummonVampire
        ));
        assert!(!gateway_zone_magic_requires_item_consumption(
            Spell::Stonetrap
        ));
        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let zone_object_id = {
            let key = runtime
                .current_presence_key()
                .expect("started runtime should have a Zone presence key");
            runtime
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .players
                .get(&key)
                .expect("started runtime should have Zone presence")
                .zone_object_id
        };
        let target = Point {
            x: self_entity.x + 1,
            y: self_entity.y,
        };
        let attack = ZoneNativePlayerAttack {
            object_id: 0,
            direction: MirDirection::Right,
            level: 2,
            damage: 0,
            monster: None,
            kind: ZoneNativePlayerAttackKind::Magic {
                target: target.clone(),
                spell: Spell::SummonSkeleton,
                cast: true,
                mp_cost: 0,
                cooldown_ms: 1,
            },
        };

        let cast_packets = runtime.execute_zone_native_player_attack(attack.clone());
        assert_eq!(
            *skill_item_calls
                .lock()
                .expect("skill item calls should lock"),
            1
        );
        assert!(commands
            .lock()
            .expect("account inventory commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-summon-item-boundary"
                    && matches!(
                        &envelope.command,
                        SharedAccountInventoryCommand::SkillItemConsume {
                            spell: Spell::SummonSkeleton,
                            request_id: 1,
                        }
                    )
            }));
        assert!(cast_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::SummonSkeleton,
                target_id: 0,
                target: packet_target,
                ..
            } if packet_target == &target
        )));
        assert!(cast_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::DeleteItem { .. })));
        assert!(!cast_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectMonster { .. })));

        let spawn_packets = runtime.dispatch_zone_player_command(
            ZoneCommand::Tick {
                now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms().saturating_add(2_000),
            },
            false,
        );
        let summon_object_id = spawn_packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::ObjectMonster { info }
                    if info.name == "BoneFamiliar"
                        && info.master_object_id == zone_object_id
                        && info.extra =>
                {
                    Some(info.object_id)
                }
                _ => None,
            })
            .expect("summon tick should spawn owned BoneFamiliar");
        assert!(spawn_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.name == "BoneFamiliar"
                    && info.master_object_id == zone_object_id
                    && info.extra
        )));

        let zone_session_id = runtime
            .current_zone_session_id()
            .expect("started runtime should have a Zone session id");
        assert!(
            !runtime.zone_native_player_attack_requires_item_consumption(&zone_session_id, &attack),
            "recasting an active Zone summon should recall it without consuming another item"
        );
        thread::sleep(Duration::from_millis(650));
        let recall_packets = runtime.execute_zone_native_player_attack(attack);
        assert_eq!(
            *skill_item_calls
                .lock()
                .expect("skill item calls should lock"),
            1,
            "Zone summon recall should not issue a second item-consumption commit"
        );
        assert!(!recall_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::DeleteItem { .. })));
        assert!(!recall_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectMonster { .. })));
        assert!(recall_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == summon_object_id
                    && movement.position == (Point {
                        x: self_entity.x,
                        y: self_entity.y
                    })
        )));
    }

    #[derive(Debug)]
    struct RecordingNpcWorldService {
        commands: Arc<Mutex<Vec<SharedNpcWorldCommandEnvelope>>>,
        committed: bool,
    }

    impl SharedNpcWorldService for RecordingNpcWorldService {
        fn commit(
            &self,
            envelope: SharedNpcWorldCommandEnvelope,
        ) -> SharedNpcWorldTransactionReceipt {
            let command = envelope.command.clone();
            self.commands
                .lock()
                .expect("npc world commands should lock")
                .push(envelope);
            if self.committed {
                SharedNpcWorldTransactionReceipt::committed(command)
            } else {
                SharedNpcWorldTransactionReceipt::rejected(command)
            }
        }
    }

    #[test]
    fn shared_in_process_runtime_uses_npc_world_service_boundary() {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let npc_service = Arc::new(RecordingNpcWorldService {
            commands: commands.clone(),
            committed: true,
        }) as SharedNpcWorldServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime_with_services(
            zone_state.clone(),
            Arc::new(InProcessAccountInventoryService::new()),
            npc_service,
        );
        start_new_runtime(&mut runtime, "zone-npc-world-service", "Scout");

        let saved = SharedNpcSavedValue {
            file_name: "quests\\flags.txt".to_string(),
            group: "profile".to_string(),
            key: "answer".to_string(),
            value: "Scout".to_string(),
        };
        runtime
            .inner
            .apply_shared_npc_saved_values(std::slice::from_ref(&saved));
        runtime.publish_shared_npc_saved_values_from_local();

        assert!(commands
            .lock()
            .expect("npc world commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-npc-world-service"
                    && envelope.identity.character_name == "Scout"
                    && matches!(
                        &envelope.command,
                        SharedNpcWorldCommand::SyncSavedValues(values)
                            if values == &vec![saved.clone()]
                    )
            }));
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_saved_values(),
            vec![saved]
        );

        runtime.inner.apply_shared_npc_random_seed(9001);
        runtime.publish_shared_npc_random_seed_from_local();
        assert!(commands
            .lock()
            .expect("npc world commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-npc-world-service"
                    && envelope.identity.character_name == "Scout"
                    && matches!(
                        &envelope.command,
                        SharedNpcWorldCommand::SyncRandomSeed(seed) if *seed == 9001
                    )
            }));
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_random_seed(),
            Some(9001)
        );

        let side_effect_packets = vec![ServerPacket::ObjectRemove { object_id: 77 }];
        let committed_packets = runtime.commit_shared_npc_entity_side_effect_packets(
            "0".to_string(),
            side_effect_packets.clone(),
        );
        assert_eq!(committed_packets, side_effect_packets);
        assert!(commands
            .lock()
            .expect("npc world commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "zone-npc-world-service"
                    && envelope.identity.character_name == "Scout"
                    && matches!(
                        &envelope.command,
                        SharedNpcWorldCommand::ApplyEntitySideEffects {
                            map_file_name,
                            packets
                        } if map_file_name == "0"
                            && packets == &vec![ServerPacket::ObjectRemove { object_id: 77 }]
                    )
            }));
    }

    #[test]
    fn shared_in_process_runtime_commits_npc_script_outcome_atomically() {
        let rejected_commands = Arc::new(Mutex::new(Vec::new()));
        let rejected_service = Arc::new(RecordingNpcWorldService {
            commands: rejected_commands.clone(),
            committed: false,
        }) as SharedNpcWorldServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut rejected_runtime = shared_session_runtime_with_services(
            zone_state.clone(),
            Arc::new(InProcessAccountInventoryService::new()),
            rejected_service,
        );
        start_new_runtime(&mut rejected_runtime, "npc-atomic-rejected", "Scout");

        let saved = SharedNpcSavedValue {
            file_name: "quests\\flags.txt".to_string(),
            group: "profile".to_string(),
            key: "answer".to_string(),
            value: "Scout".to_string(),
        };
        let side_effect = SharedNpcEntitySideEffect {
            map_file_name: "0".to_string(),
            packets: vec![ServerPacket::ObjectRemove { object_id: 77 }],
        };
        let rejected_packets = rejected_runtime.commit_shared_npc_script_outcome(
            vec![saved.clone()],
            9001,
            Some(side_effect.clone()),
        );

        assert!(rejected_packets.is_empty());
        assert!(rejected_commands
            .lock()
            .expect("npc world commands should lock")
            .iter()
            .any(|envelope| {
                envelope.identity.account_id == "npc-atomic-rejected"
                    && matches!(
                        &envelope.command,
                        SharedNpcWorldCommand::ApplyScriptOutcome {
                            saved_values,
                            random_seed,
                            entity_side_effect
                        } if saved_values == &vec![saved.clone()]
                            && *random_seed == 9001
                            && entity_side_effect.as_ref() == Some(&side_effect)
                    )
            }));
        assert!(zone_state
            .lock()
            .expect("shared zone state should lock")
            .shared_npc_saved_values()
            .is_empty());
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_random_seed(),
            None
        );

        let committed_commands = Arc::new(Mutex::new(Vec::new()));
        let committed_service = Arc::new(RecordingNpcWorldService {
            commands: committed_commands,
            committed: true,
        }) as SharedNpcWorldServiceHandle;
        let mut committed_runtime = shared_session_runtime_with_services(
            zone_state.clone(),
            Arc::new(InProcessAccountInventoryService::new()),
            committed_service,
        );
        start_new_runtime(&mut committed_runtime, "npc-atomic-committed", "Blade");

        let committed_packets = committed_runtime.commit_shared_npc_script_outcome(
            vec![saved.clone()],
            9001,
            Some(side_effect.clone()),
        );

        assert_eq!(committed_packets, side_effect.packets);
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_saved_values(),
            vec![saved]
        );
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_random_seed(),
            Some(9001)
        );
    }

    #[test]
    fn shared_zone_state_dispatches_current_and_pending_player_damages() {
        let mut state = SharedInProcessZoneState::new();
        let current_key = ZonePresenceKey {
            account_id: "current".to_string(),
            character_index: 0,
        };
        let remote_key = ZonePresenceKey {
            account_id: "remote".to_string(),
            character_index: 0,
        };
        let current_session_id = SharedInProcessZoneState::zone_session_id_for_key(&current_key);
        let remote_session_id = SharedInProcessZoneState::zone_session_id_for_key(&remote_key);
        state
            .zone_session_keys
            .insert(current_session_id.clone(), current_key.clone());
        state
            .zone_session_keys
            .insert(remote_session_id.clone(), remote_key.clone());

        let (_, _, _, _, _, current_damages, _) = state.dispatch_zone_outbounds(
            vec![
                ZoneOutbound::PlayerDamaged {
                    session_id: current_session_id,
                    damage: 3,
                },
                ZoneOutbound::PlayerDamaged {
                    session_id: remote_session_id,
                    damage: 5,
                },
            ],
            Some(&current_key),
        );

        assert_eq!(current_damages, vec![3]);
        assert_eq!(state.take_pending_zone_player_damages(&remote_key), vec![5]);
    }

    #[test]
    fn shared_zone_state_dispatches_current_and_pending_player_heals() {
        let mut state = SharedInProcessZoneState::new();
        let current_key = ZonePresenceKey {
            account_id: "current".to_string(),
            character_index: 0,
        };
        let remote_key = ZonePresenceKey {
            account_id: "remote".to_string(),
            character_index: 0,
        };
        let current_session_id = SharedInProcessZoneState::zone_session_id_for_key(&current_key);
        let remote_session_id = SharedInProcessZoneState::zone_session_id_for_key(&remote_key);
        state
            .zone_session_keys
            .insert(current_session_id.clone(), current_key.clone());
        state
            .zone_session_keys
            .insert(remote_session_id.clone(), remote_key.clone());

        let (_, _, _, _, _, _, current_heals) = state.dispatch_zone_outbounds(
            vec![
                ZoneOutbound::PlayerHealed {
                    session_id: current_session_id,
                    amount: 3,
                },
                ZoneOutbound::PlayerHealed {
                    session_id: remote_session_id,
                    amount: 5,
                },
            ],
            Some(&current_key),
        );

        assert_eq!(current_heals, vec![3]);
        assert_eq!(state.take_pending_zone_player_heals(&remote_key), vec![5]);
    }

    #[test]
    fn shared_in_process_runtime_applies_pending_zone_player_damage_to_session_hp() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut runtime);
        let starting_hp = runtime
            .world_snapshot()
            .player_hp
            .expect("started runtime should expose player hp");
        let key = runtime
            .current_presence_key()
            .expect("started runtime should have presence key");
        let session_id = SharedInProcessZoneState::zone_session_id_for_key(&key);
        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            let _ = state.dispatch_zone_outbounds(
                vec![ZoneOutbound::PlayerDamaged {
                    session_id,
                    damage: 4,
                }],
                None,
            );
        }

        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 99,
            }))
            .expect("keepalive should drain pending zone damage");

        assert_eq!(
            runtime.world_snapshot().player_hp,
            Some((starting_hp - 4).max(1))
        );
    }

    #[test]
    fn shared_in_process_runtime_applies_pending_zone_player_heal_to_session_hp() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut runtime);
        let starting_hp = runtime
            .world_snapshot()
            .player_hp
            .expect("started runtime should expose player hp");
        runtime.inner.apply_zone_player_damage(10);
        let damaged_hp = runtime
            .world_snapshot()
            .player_hp
            .expect("damaged runtime should expose player hp");
        let key = runtime
            .current_presence_key()
            .expect("started runtime should have presence key");
        let session_id = SharedInProcessZoneState::zone_session_id_for_key(&key);
        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            let _ = state.dispatch_zone_outbounds(
                vec![ZoneOutbound::PlayerHealed {
                    session_id,
                    amount: 4,
                }],
                None,
            );
        }

        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 99,
            }))
            .expect("keepalive should drain pending zone heal");

        assert_eq!(
            runtime.world_snapshot().player_hp,
            Some((damaged_hp + 4).min(starting_hp))
        );
    }

    #[test]
    fn shared_in_process_runtime_mirrors_pending_zone_self_buff_to_session_snapshot() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut runtime);
        let key = runtime
            .current_presence_key()
            .expect("started runtime should have presence key");
        let session_id = SharedInProcessZoneState::zone_session_id_for_key(&key);
        let zone_object_id = zone_state
            .lock()
            .expect("shared zone state should lock")
            .players
            .get(&key)
            .expect("started runtime should have zone presence")
            .zone_object_id;

        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            let _ = state.dispatch_zone_outbounds(
                vec![ZoneOutbound::ToSession {
                    session_id: session_id.clone(),
                    packets: vec![ServerPacket::AddBuff {
                        buff: ClientBuff {
                            buff_type: 24,
                            visible: true,
                            object_id: zone_object_id,
                            expire_time: 30_000,
                            infinite: false,
                            paused: false,
                            stats: vec![UserItemStat {
                                stat: 124,
                                value: 30,
                            }],
                            values: Vec::new(),
                        },
                    }],
                }],
                None,
            );
        }

        let add_packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 100,
            }))
            .expect("keepalive should drain pending zone buff");

        assert!(add_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == zone_object_id
                    && buff.buff_type == 24
                    && buff.stats.iter().any(|stat| stat.stat == 124 && stat.value == 30)
        )));
        let snapshot = runtime.world_snapshot();
        let mirrored_buff = snapshot
            .active_buffs
            .iter()
            .find(|buff| buff.key == "magic-shield")
            .expect("zone self AddBuff should mirror into personal BuffResource");
        assert!(mirrored_buff.remaining_ticks > 0);

        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            let _ = state.dispatch_zone_outbounds(
                vec![ZoneOutbound::ToSession {
                    session_id,
                    packets: vec![ServerPacket::RemoveBuff {
                        object_id: zone_object_id,
                        buff_type: 24,
                    }],
                }],
                None,
            );
        }

        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 101,
            }))
            .expect("keepalive should drain pending zone buff removal");
        assert!(!runtime
            .world_snapshot()
            .active_buffs
            .iter()
            .any(|buff| buff.key == "magic-shield"));
    }

    #[test]
    fn delayed_player_action_packets_keep_only_owned_tick_damage_bundle() {
        let packets = vec![
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: 77,
                    attacker_id: 101,
                    location: Point { x: 331, y: 270 },
                    direction: MirDirection::Left,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 80,
                    expire: 0,
                },
            },
            ServerPacket::ObjectPoisoned {
                object_id: 77,
                poison: 3,
            },
            ServerPacket::AddBuff {
                buff: mir2_protocol::ClientBuff {
                    buff_type: 7,
                    visible: true,
                    object_id: 77,
                    expire_time: 500,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: vec![1],
                },
            },
            ServerPacket::RemoveBuff {
                object_id: 77,
                buff_type: 7,
            },
            ServerPacket::PauseBuff {
                object_id: 77,
                buff_type: 8,
                paused: true,
            },
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: 88,
                    attacker_id: 202,
                    location: Point { x: 332, y: 270 },
                    direction: MirDirection::Left,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 88,
                    percent: 60,
                    expire: 0,
                },
            },
            ServerPacket::ObjectPoisoned {
                object_id: 88,
                poison: 5,
            },
            ServerPacket::AddBuff {
                buff: mir2_protocol::ClientBuff {
                    buff_type: 9,
                    visible: true,
                    object_id: 88,
                    expire_time: 500,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: vec![2],
                },
            },
        ];

        let filtered = delayed_player_action_packets(101, &packets);

        assert!(filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == 77 && info.attacker_id == 101
        )));
        assert!(filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 77 && info.percent == 80
        )));
        assert!(filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == 77 && *poison == 3
        )));
        assert!(filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == 77 && buff.buff_type == 7
        )));
        assert!(filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::RemoveBuff { object_id, buff_type }
                if *object_id == 77 && *buff_type == 7
        )));
        assert!(filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::PauseBuff {
                object_id,
                buff_type,
                paused,
            } if *object_id == 77 && *buff_type == 8 && *paused
        )));
        assert!(!filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == 88
        )));
        assert!(!filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 88
        )));
        assert!(!filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, .. } if *object_id == 88
        )));
        assert!(!filtered.iter().any(|packet| matches!(
            packet,
            ServerPacket::AddBuff { buff } if buff.object_id == 88
        )));
    }

    #[test]
    fn shared_zone_state_applies_object_health_without_stale_overwrite() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(100);
        entity.max_hp = Some(100);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 40,
                    expire: 0,
                },
            }],
        );
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let hp = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .get(&77)
            .and_then(|entity| entity.hp);
        assert_eq!(hp, Some(40));
    }

    #[test]
    fn shared_zone_state_keeps_lower_health_against_stale_packet_increase() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(100);
        entity.max_hp = Some(100);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 40,
                    expire: 0,
                },
            }],
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 80,
                    expire: 0,
                },
            }],
        );

        let hp = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .get(&77)
            .and_then(|entity| entity.hp);
        assert_eq!(hp, Some(40));
    }

    #[test]
    fn shared_zone_state_treats_zero_health_as_dead() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(100);
        entity.max_hp = Some(100);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 0,
                    expire: 0,
                },
            }],
        );
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        let entity = map.entities.get(&77).expect("dead body should remain");
        assert!(entity.dead);
        assert_eq!(entity.hp, Some(0));
        assert!(!state.shared_entity_allows_action("0", 77));
    }

    #[test]
    fn shared_zone_state_treats_zero_health_without_max_hp_as_dead() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = None;
        entity.max_hp = None;

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 0,
                    expire: 0,
                },
            }],
        );
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("dead body should remain even without max hp");
        assert!(entity.dead);
        assert_eq!(entity.hp, Some(0));
        assert!(!state.shared_entity_allows_action("0", 77));
    }

    #[test]
    fn shared_zone_state_commits_monster_death_drops_once() {
        let mut state = SharedInProcessZoneState::new();
        let entity = shared_monster_entity(77);
        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let death_packets = vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 77,
                location: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
                kind: 0,
            },
        }];
        state.apply_shared_entity_packets("0", &death_packets);
        let committed = state.commit_death_drops(
            "0",
            &death_packets,
            &[shared_gold_drop(88, 329, 269, None, None)],
        );
        let duplicate = state.commit_death_drops(
            "0",
            &death_packets,
            &[shared_gold_drop(89, 330, 269, None, None)],
        );
        assert_eq!(
            committed
                .iter()
                .map(|drop| drop.object_id)
                .collect::<Vec<_>>(),
            vec![88]
        );
        assert!(duplicate.is_empty());

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        assert!(map.ground_drops.contains_key(&88));
        assert!(!map.ground_drops.contains_key(&89));

        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            vec![shared_gold_drop(89, 330, 269, None, None)],
            BTreeSet::new(),
        );
        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        assert!(map.ground_drops.contains_key(&88));
        assert!(!map.ground_drops.contains_key(&89));
    }

    #[test]
    fn shared_zone_state_death_drop_anchor_survives_corpse_remove() {
        let mut state = SharedInProcessZoneState::new();
        let entity = shared_monster_entity(77);
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let death_packets = vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 77,
                location: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
                kind: 0,
            },
        }];
        state.apply_shared_entity_packets("0", &death_packets);
        let committed = state.commit_death_drops(
            "0",
            &death_packets,
            &[shared_gold_drop(88, 329, 269, None, None)],
        );
        assert_eq!(committed.len(), 1);

        state.apply_shared_entity_packets("0", &[ServerPacket::ObjectRemove { object_id: 77 }]);
        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::from([77]),
            vec![shared_gold_drop(89, 330, 269, None, None)],
            BTreeSet::new(),
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        assert!(map.ground_drops.contains_key(&88));
        assert!(!map.ground_drops.contains_key(&89));
    }

    #[test]
    fn shared_zone_state_commits_zero_health_monster_drops_once() {
        let mut state = SharedInProcessZoneState::new();
        let entity = shared_monster_entity(77);
        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let death_packets = vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 77,
                percent: 0,
                expire: 0,
            },
        }];
        state.apply_shared_entity_packets("0", &death_packets);
        let committed = state.commit_death_drops(
            "0",
            &death_packets,
            &[shared_gold_drop(88, 329, 269, None, None)],
        );
        let duplicate = state.commit_death_drops(
            "0",
            &death_packets,
            &[shared_gold_drop(89, 330, 269, None, None)],
        );
        assert_eq!(
            committed
                .iter()
                .map(|drop| drop.object_id)
                .collect::<Vec<_>>(),
            vec![88]
        );
        assert!(duplicate.is_empty());

        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            vec![shared_gold_drop(89, 330, 269, None, None)],
            BTreeSet::new(),
        );
        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        assert!(map.ground_drops.contains_key(&88));
        assert!(!map.ground_drops.contains_key(&89));
    }

    #[test]
    fn shared_death_drop_spawn_packet_uses_committed_drop_payload() {
        let packet = ground_drop_spawn_packet(&shared_gold_drop(88, 329, 269, None, None));

        assert!(matches!(
            packet,
            ServerPacket::ObjectGold { info }
                if info.object_id == 88
                    && info.gold == 25
                    && info.location == (Point { x: 329, y: 269 })
        ));
    }

    #[test]
    fn shared_zone_state_applies_object_died_without_stale_revive() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(100);
        entity.max_hp = Some(100);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 77,
                    location: Point { x: 330, y: 271 },
                    direction: MirDirection::Up,
                    kind: 0,
                },
            }],
        );
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        let entity = map.entities.get(&77).expect("dead body should remain");
        assert!(entity.dead);
        assert_eq!(entity.hp, Some(0));
        assert_eq!(entity.x, 330);
        assert_eq!(entity.y, 271);
        assert_eq!(entity.direction, MirDirection::Up);
    }

    #[test]
    fn shared_zone_state_applies_object_died_before_snapshot_arrives() {
        let mut state = SharedInProcessZoneState::new();
        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 77,
                    location: Point { x: 330, y: 271 },
                    direction: MirDirection::Up,
                    kind: 0,
                },
            }],
        );
        assert!(!state.shared_entity_allows_action("0", 77));

        let mut stale_entity = shared_monster_entity(77);
        stale_entity.hp = Some(12);
        stale_entity.max_hp = Some(12);
        state.sync_map_layer(
            "0".to_string(),
            vec![stale_entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("dead entity should be materialized from later snapshot");
        assert!(entity.dead);
        assert_eq!(entity.hp, Some(0));
        assert_eq!(entity.x, 330);
        assert_eq!(entity.y, 271);
        assert_eq!(entity.direction, MirDirection::Up);
    }

    #[test]
    fn shared_zone_state_commits_death_drop_without_prior_entity_snapshot() {
        let mut state = SharedInProcessZoneState::new();
        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        let death_packets = vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 77,
                location: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
                kind: 0,
            },
        }];
        state.apply_shared_entity_packets("0", &death_packets);

        let committed = state.commit_death_drops(
            "0",
            &death_packets,
            &[shared_gold_drop(88, 329, 269, None, None)],
        );

        assert_eq!(
            committed
                .iter()
                .map(|drop| drop.object_id)
                .collect::<Vec<_>>(),
            vec![88]
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .ground_drops
            .contains_key(&88));
    }

    #[test]
    fn shared_zone_state_applies_object_revived_without_stale_redeath() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(0);
        entity.dead = true;

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHarvested {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 329, y: 269 },
                    direction: MirDirection::Down,
                },
            }],
        );
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectRevived {
                info: ObjectRevivedInfo {
                    object_id: 77,
                    effect: true,
                },
            }],
        );
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should exist");
        let entity = map
            .entities
            .get(&77)
            .expect("revived monster should remain");
        assert!(!entity.dead);
        assert_eq!(entity.hp, Some(12));
        assert!(!map.harvested_entity_ids.contains(&77));
        assert!(state.shared_entity_allows_action("0", 77));
    }

    #[test]
    fn shared_zone_state_object_revived_clears_remove_tombstone() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(12);
        entity.dead = false;

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets("0", &[ServerPacket::ObjectRemove { object_id: 77 }]);
        assert!(state.shared_entity("0", 77).is_none());

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectRevived {
                info: ObjectRevivedInfo {
                    object_id: 77,
                    effect: true,
                },
            }],
        );
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("revived object should clear remove tombstone");
        assert!(!entity.dead);
        assert_eq!(entity.hp, Some(12));
    }

    #[test]
    fn shared_zone_state_applies_object_revived_before_snapshot_arrives() {
        let mut state = SharedInProcessZoneState::new();
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectRevived {
                info: ObjectRevivedInfo {
                    object_id: 77,
                    effect: true,
                },
            }],
        );
        let mut stale_dead = shared_monster_entity(77);
        stale_dead.hp = Some(0);
        stale_dead.dead = true;

        state.sync_map_layer(
            "0".to_string(),
            vec![stale_dead],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        let entity = state
            .shared_entity("0", 77)
            .expect("revived marker should apply to later snapshot");
        assert!(!entity.dead);
        assert_eq!(entity.hp, Some(12));
        assert!(state.shared_entity_allows_action("0", 77));
    }

    #[test]
    fn shared_zone_state_applies_object_remove_as_entity_tombstone() {
        let mut state = SharedInProcessZoneState::new();
        let entity = shared_monster_entity(77);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.apply_shared_entity_packets("0", &[ServerPacket::ObjectRemove { object_id: 77 }]);
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));
        assert!(!state.shared_entity_allows_action("0", 77));
    }

    #[test]
    fn shared_zone_state_blocks_actions_against_dead_entities() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(100);
        entity.max_hp = Some(100);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(state.shared_entity_allows_action("0", 77));

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 77,
                    location: Point { x: 330, y: 271 },
                    direction: MirDirection::Up,
                    kind: 0,
                },
            }],
        );
        assert!(!state.shared_entity_allows_action("0", 77));
    }

    #[test]
    fn shared_zone_state_blocks_reharvesting_harvested_shared_corpse() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(0);
        entity.dead = true;
        let picker = shared_picker_entity(101, 328, 269);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(state.shared_harvest_allows_action("0", &picker, MirDirection::Right));

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHarvested {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 329, y: 269 },
                    direction: MirDirection::Down,
                },
            }],
        );
        assert!(!state.shared_harvest_allows_action("0", &picker, MirDirection::Right));

        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(!state.shared_harvest_allows_action("0", &picker, MirDirection::Right));
    }

    #[test]
    fn shared_zone_state_blocks_reharvest_when_harvest_packet_arrives_before_snapshot() {
        let mut state = SharedInProcessZoneState::new();
        let picker = shared_picker_entity(101, 328, 269);
        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectHarvested {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 329, y: 269 },
                    direction: MirDirection::Down,
                },
            }],
        );

        let mut stale_dead = shared_monster_entity(77);
        stale_dead.hp = Some(0);
        stale_dead.dead = true;
        state.sync_map_layer(
            "0".to_string(),
            vec![stale_dead],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        assert!(!state.shared_harvest_allows_action("0", &picker, MirDirection::Right));
    }

    #[test]
    fn shared_zone_state_respects_shared_drop_ownership_window() {
        let mut state = SharedInProcessZoneState::new();
        let picker = shared_picker_entity(9001, 330, 270);
        let owned_drop = shared_gold_drop(88, 330, 270, Some(101), Some(60));

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![owned_drop],
            BTreeSet::new(),
        );

        assert_eq!(
            state.take_pickable_drop("0", Some(88), &picker, 102, &[]),
            SharedDropPickupResult::OwnerBlocked
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map should exist")
            .ground_drops
            .contains_key(&88));
        assert!(matches!(
            state.take_pickable_drop("0", Some(88), &picker, 101, &[]),
            SharedDropPickupResult::Picked(_)
        ));
    }

    #[test]
    fn shared_zone_state_allows_shared_drop_owner_group_member() {
        let mut state = SharedInProcessZoneState::new();
        let owner_key = ZonePresenceKey {
            account_id: "owner".to_string(),
            character_index: 0,
        };
        let owner_zone_object_id = state.upsert_player(
            owner_key,
            "Owner",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        let picker = shared_picker_entity(102, 330, 270);
        let owned_drop = shared_gold_drop(88, 330, 270, Some(owner_zone_object_id), Some(60));

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![owned_drop],
            BTreeSet::new(),
        );

        assert!(matches!(
            state.take_pickable_drop("0", Some(88), &picker, 50_999, &["Owner".to_string()]),
            SharedDropPickupResult::Picked(_)
        ));
    }

    #[test]
    fn shared_zone_state_allows_shared_drop_after_ownership_window() {
        let mut state = SharedInProcessZoneState::new();
        let picker = shared_picker_entity(9001, 330, 270);
        let expired_drop = shared_gold_drop(88, 330, 270, Some(101), None);

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![expired_drop],
            BTreeSet::new(),
        );

        assert!(matches!(
            state.take_pickable_drop("0", Some(88), &picker, 102, &[]),
            SharedDropPickupResult::Picked(_)
        ));
    }

    #[test]
    fn shared_zone_state_expires_shared_drop_ownership_deadline_before_pickup() {
        let mut state = SharedInProcessZoneState::new();
        let picker = shared_picker_entity(9001, 330, 270);
        let owned_drop = shared_gold_drop(88, 330, 270, Some(101), Some(60));

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![owned_drop],
            BTreeSet::new(),
        );
        state
            .maps
            .get_mut("0")
            .expect("shared map should exist")
            .drop_ownership_expires_at_ms
            .insert(88, 0);

        let pickup = state.take_pickable_drop("0", Some(88), &picker, 102, &[]);

        assert!(matches!(
            pickup,
            SharedDropPickupResult::Picked(GroundDropSnapshot {
                ownership_remaining_ticks: None,
                ..
            })
        ));
    }

    #[test]
    fn shared_zone_state_expires_shared_drop_ownership_deadline_before_auto_creature_pickup() {
        let mut state = SharedInProcessZoneState::new();
        let picker = shared_picker_entity(102, 330, 270);
        let owned_drop = shared_gold_drop(88, 331, 270, Some(101), Some(60));

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![owned_drop],
            BTreeSet::new(),
        );
        state
            .maps
            .get_mut("0")
            .expect("shared map should exist")
            .drop_ownership_expires_at_ms
            .insert(88, 0);

        assert!(matches!(
            state.take_auto_pickable_drop_for_creature(
                "0",
                &Point {
                    x: picker.x,
                    y: picker.y
                },
                102,
                &[],
                &shared_pickup_creature(),
            ),
            SharedDropPickupResult::Picked(GroundDropSnapshot {
                ownership_remaining_ticks: None,
                ..
            })
        ));
    }

    #[test]
    fn shared_zone_state_expires_shared_drop_and_queues_object_remove() {
        let mut state = SharedInProcessZoneState::new();
        let current_key = ZonePresenceKey {
            account_id: "owner".to_string(),
            character_index: 0,
        };
        let observer_key = ZonePresenceKey {
            account_id: "observer".to_string(),
            character_index: 0,
        };
        state.upsert_player(
            current_key.clone(),
            "Scout",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        state.upsert_player(
            observer_key.clone(),
            "Blade",
            "0".to_string(),
            shared_picker_entity(102, 331, 270),
            80,
        );
        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::new(),
            vec![shared_gold_drop(88, 330, 270, Some(101), Some(60))],
            BTreeSet::new(),
        );
        state
            .maps
            .get_mut("0")
            .expect("shared map should exist")
            .drop_expires_at_ms
            .insert(88, 0);

        let current_packets =
            state.expire_shared_drops("0", Some(&current_key), shared_gateway_now_ms());
        let observer_packets = state.take_pending_zone_packets(&observer_key);
        let map = state
            .map_layer(Some("0"))
            .expect("shared map should still exist after expiring a drop");

        assert!(current_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 88
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 88
        )));
        assert!(!map.ground_drops.contains_key(&88));
        assert!(map.removed_drop_ids.contains(&88));
        assert!(!map.drop_ownership_expires_at_ms.contains_key(&88));
        assert!(!map.drop_expires_at_ms.contains_key(&88));
    }

    #[test]
    fn shared_in_process_registry_surfaces_remote_players_in_snapshots() {
        let (first, second) = started_shared_zone_sessions();

        let first_snapshot = first.world_snapshot();
        let second_snapshot = second.world_snapshot();

        assert!(first_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Blade"
        }));
        assert!(second_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Scout"
        }));
    }

    #[test]
    fn shared_in_process_registry_routes_walk_through_shared_zone() {
        let (mut first, mut second) = started_shared_zone_sessions();

        let owner_packets = first.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        assert!(owner_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));

        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 100 });
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.direction == MirDirection::Right
        )));
    }

    #[test]
    fn shared_zone_upsert_preserves_same_map_authoritative_transform() {
        let mut state = SharedInProcessZoneState::new();
        let key = ZonePresenceKey {
            account_id: "same-map-transform".to_string(),
            character_index: 0,
        };

        let zone_object_id = state.upsert_player(
            key.clone(),
            "Scout",
            "0".to_string(),
            shared_picker_entity(1000, 330, 270),
            20,
        );
        state.update_player_transform(&key, Point { x: 339, y: 271 }, MirDirection::Up);

        let stale_upsert_object_id = state.upsert_player(
            key.clone(),
            "Scout",
            "0".to_string(),
            shared_picker_entity(1000, 330, 270),
            18,
        );

        let presence = state
            .players
            .get(&key)
            .expect("player presence should remain registered");
        assert_eq!(stale_upsert_object_id, zone_object_id);
        assert_eq!((presence.entity.x, presence.entity.y), (339, 271));
        assert_eq!(presence.entity.direction, MirDirection::Up);
        assert_eq!(presence.free_bag_slots, 18);
    }

    #[test]
    fn shared_in_process_registry_routes_monster_attack_through_zone_native_combat() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let snapshot = first.world_snapshot();
        let map_file_name = snapshot
            .map_file_name
            .clone()
            .expect("first session should be in a map");
        let monster = snapshot
            .entities
            .iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.disposition == WorldEntityDisposition::Hostile
                    && !entity.dead
            })
            .cloned()
            .expect("starter scene should expose a live monster");
        assert!(
            !map_file_name.is_empty(),
            "first session should report a non-empty map"
        );
        let mut owner_packets = Vec::new();
        // Crystal-faithful zone melee (PR #21) lowers per-hit damage, so the
        // level-7 starter now needs ~11 hits to kill the starter monster (was
        // <=8). Budget with headroom; the loop short-circuits on ObjectDied, so a
        // larger safety cap costs nothing at runtime.
        for tick in 0..32 {
            owner_packets.extend(first.attack(monster.object_id));
            owner_packets.extend(first.handle_packet(ClientPacket::KeepAlive { time: tick }));
            if owner_packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectDied { info } if info.object_id == monster.object_id
                )
            }) {
                break;
            }
        }
        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 100 });

        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id != monster.object_id
        )));
        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == monster.object_id
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == monster.object_id
        )));
    }

    #[test]
    fn shared_in_process_registry_same_map_transfer_syncs_zone_movement_origin() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);

        let transfer_packets = session.transfer_map("crystal:0:330:270");
        assert!(transfer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position.x == 330 && location.position.y == 270
        )));

        let owner_packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });

        assert!(
            owner_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == 331 && location.position.y == 270
            )),
            "same-map transfer must update the shared zone origin before movement: {owner_packets:?}"
        );
    }

    #[test]
    fn shared_in_process_crystal_runtime_does_not_apply_starter_demo_gate_transfer() {
        let registry = ZoneRegistry::in_process();
        let mut session = GatewaySession::new_with_zone_registry(
            GatewayConfig::default().with_crystal_map_runtime(),
            &registry,
        );
        start_demo_character(&mut session);
        session.transfer_map("crystal:0:338:270");

        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });

        assert!(
            !packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })),
            "Crystal runtime should not use the starter demo same-map gate: {packets:?}"
        );
        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == 339 && location.position.y == 270
            )),
            "walking right from 338,270 should remain normal movement in Crystal runtime: {packets:?}"
        );
    }

    #[test]
    fn shared_in_process_registry_walk_onto_crystal_movement_transfers_map() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);
        session.transfer_map("crystal:0:307:264");

        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        let snapshot = session.world_snapshot();

        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::MapInformation { info }
                    if info.file_name == "0102" && info.title == "MeatStore"
            )),
            "walking onto a Crystal movement tile should transfer through the shared Zone path: {packets:?}"
        );
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position.x == 3 && location.position.y == 7
        )));
        assert_eq!(snapshot.map_file_name.as_deref(), Some("0102"));
    }

    #[test]
    fn shared_in_process_registry_walk_onto_library_movement_transfers_map() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);
        session.transfer_map("crystal:0:322:248");

        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Up,
        });
        let snapshot = session.world_snapshot();

        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::MapInformation { info }
                    if info.file_name == "0104" && info.title == "Library"
            )),
            "walking onto the Bichon Library movement tile should transfer through the shared Zone path: {packets:?}"
        );
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position.x == 4 && location.position.y == 10
        )));
        assert_eq!(snapshot.map_file_name.as_deref(), Some("0104"));
    }

    #[test]
    fn shared_in_process_registry_consumes_nearly_ready_crystal_run_chain() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);
        session.transfer_map("crystal:0102:3:7");

        let first_packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        assert!(first_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position.x == 4 && location.position.y == 7
        )));

        thread::sleep(Duration::from_millis(520));
        let second_packets = session.handle_packet(ClientPacket::Run {
            direction: MirDirection::Right,
        });

        assert!(
            second_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == 6 && location.position.y == 7
            )),
            "nearly-ready run intent should not wait for a later world tick: {second_packets:?}"
        );
    }

    #[test]
    fn shared_in_process_registry_routes_run_through_shared_zone() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.transfer_map("crystal:0102:3:7");
        second.transfer_map("crystal:0102:9:7");

        first.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        thread::sleep(Duration::from_millis(650));

        let owner_packets = first.handle_packet(ClientPacket::Run {
            direction: MirDirection::Right,
        });
        assert!(owner_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));

        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 102 });
        assert!(
            observer_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectRun { movement } if movement.direction == MirDirection::Right
            )),
            "expected ObjectRun in observer packets: {observer_packets:?}"
        );
    }

    #[test]
    fn shared_in_process_registry_routes_turn_through_shared_zone() {
        let (mut first, mut second) = started_shared_zone_sessions();

        let owner_packets = first.handle_packet(ClientPacket::Turn {
            direction: MirDirection::Left,
        });
        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location } if location.direction == MirDirection::Left
        )));

        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 103 });
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectTurn { movement } if movement.direction == MirDirection::Left
        )));
    }

    #[test]
    fn shared_in_process_registry_routes_chat_through_shared_zone() {
        let (mut first, mut second) = started_shared_zone_sessions();

        let owner_packets = first.handle_packet(ClientPacket::Chat {
            message: "shared hello".to_string(),
            linked_items: Vec::new(),
        });
        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { text, .. } if text.contains("shared hello")
        )));

        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 104 });
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { text, .. } if text.contains("shared hello")
        )));
    }

    #[test]
    fn shared_in_process_runtime_broadcasts_delayed_player_tick_damage() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state);
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "second-delayed", "Blade");
        let target = first
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live monster");
        first.inner.force_authoritative_player_transform(
            Point {
                x: target.x.saturating_sub(1),
                y: target.y,
            },
            MirDirection::Right,
        );
        first.sync_zone_snapshot();
        second.sync_zone_snapshot();
        let first_zone_object_id = second
            .world_snapshot()
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::Player && entity.name == "Scout")
            .map(|entity| entity.object_id)
            .expect("second runtime should see first player");

        let launch_packets = first
            .execute(WorldCommand::Attack {
                object_id: target.object_id,
            })
            .expect("attack should execute");
        assert!(launch_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
        assert!(!launch_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectHealth { .. })));

        let owner_tick_packets = first
            .execute(WorldCommand::Tick)
            .expect("owner tick should execute");
        assert!(owner_tick_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == target.object_id
        )));
        let observer_packets = second
            .execute(WorldCommand::Tick)
            .expect("observer tick should execute");

        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == target.object_id && info.attacker_id == first_zone_object_id
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == target.object_id
        )));
    }

    #[test]
    fn shared_in_process_runtime_routes_range_attack_through_shared_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state);
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "second-range", "Blade");
        let target = first
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live monster");
        let attacker_position = Point {
            x: target.x.saturating_sub(3),
            y: target.y,
        };
        first
            .inner
            .force_authoritative_player_transform(attacker_position.clone(), MirDirection::Right);
        first.sync_zone_snapshot();
        second.sync_zone_snapshot();

        let launch_packets = first
            .execute(WorldCommand::ClientPacket(ClientPacket::RangeAttack {
                direction: MirDirection::Right,
                location: attacker_position,
                target_id: target.object_id,
                target_location: Point {
                    x: target.x,
                    y: target.y,
                },
            }))
            .expect("range attack should execute");

        assert!(launch_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::RangeAttack { target_id, .. } if *target_id == target.object_id
        )));
        assert!(launch_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { info } if info.target_id == target.object_id
        )));
        assert!(!launch_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectHealth { .. })));

        let owner_tick_packets = first
            .execute(WorldCommand::Tick)
            .expect("owner tick should execute");
        assert!(owner_tick_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == target.object_id
        )));
        let observer_packets = second
            .execute(WorldCommand::Tick)
            .expect("observer tick should execute");
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { info } if info.target_id == target.object_id
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == target.object_id
        )));
    }

    #[test]
    fn shared_in_process_runtime_rejects_early_range_attack_at_zone_boundary() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state);
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "second-range-window", "Blade");
        let target = first
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live monster");
        let attacker_position = Point {
            x: target.x.saturating_sub(3),
            y: target.y,
        };
        first
            .inner
            .force_authoritative_player_transform(attacker_position.clone(), MirDirection::Right);
        first.sync_zone_snapshot();
        second.sync_zone_snapshot();

        let first_launch = first
            .execute(WorldCommand::ClientPacket(ClientPacket::RangeAttack {
                direction: MirDirection::Right,
                location: attacker_position.clone(),
                target_id: target.object_id,
                target_location: Point {
                    x: target.x,
                    y: target.y,
                },
            }))
            .expect("first range attack should execute");
        assert!(first_launch.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { info } if info.target_id == target.object_id
        )));
        let _ = first
            .execute(WorldCommand::Tick)
            .expect("owner tick should drain first range attack");
        let _ = second
            .execute(WorldCommand::Tick)
            .expect("observer tick should drain first range attack");

        let early = first
            .execute(WorldCommand::ClientPacket(ClientPacket::RangeAttack {
                direction: MirDirection::Right,
                location: attacker_position.clone(),
                target_id: target.object_id,
                target_location: Point {
                    x: target.x,
                    y: target.y,
                },
            }))
            .expect("early range attack should be corrected by Zone");

        assert!(early
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(!early.iter().any(|packet| matches!(
            packet,
            ServerPacket::RangeAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
        )));
        let observer_packets = second
            .execute(WorldCommand::Tick)
            .expect("observer tick should execute");
        assert!(!observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { info } if info.target_id == target.object_id
        )));
    }

    #[test]
    fn shared_in_process_runtime_routes_magic_through_shared_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state);
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "magic-observer", "Blade");

        let target = first
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live monster");
        let attacker_position = Point {
            x: target.x.saturating_sub(4),
            y: target.y,
        };
        first
            .inner
            .force_authoritative_player_transform(attacker_position.clone(), MirDirection::Right);
        first.sync_zone_snapshot();
        second.sync_zone_snapshot();

        let self_object_id = first
            .world_snapshot()
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| entity.object_id)
            .expect("owner should expose self object id");
        let launch_packets = first
            .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
                object_id: self_object_id,
                spell: Spell::Healing,
                direction: MirDirection::Right,
                target_id: target.object_id,
                location: Point {
                    x: target.x,
                    y: target.y,
                },
                spell_target_lock: true,
            }))
            .expect("Healing should execute through shared Zone");

        assert!(launch_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell,
                target_id,
                ..
            } if *spell == Spell::Healing && *target_id == target.object_id
        )));
        assert!(launch_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                ..
            } if *spell == Spell::Healing && *target_id == target.object_id
        )));

        let observer_packets = second
            .execute(WorldCommand::Tick)
            .expect("observer tick should drain magic packets");
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                ..
            } if *spell == Spell::Healing && *target_id == target.object_id
        )));
    }

    #[test]
    fn shared_in_process_runtime_applies_current_map_shared_monsters_to_local_runtime() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(GatewayConfig::default()),
            zone_state: zone_state.clone(),
            account_inventory_service: Arc::new(InProcessAccountInventoryService::new()),
            npc_world_service: Arc::new(InProcessNpcWorldService),
            presence_key: None,
            zone_move_seq: 0,
            shared_skill_item_request_seq: 0,
            force_next_zone_transform_sync: false,
            cached_map_file_name: None,
            cached_local_self_object_id: None,
            cached_map_transfers: Vec::new(),
            last_shared_entity_ids_by_map: Default::default(),
            last_shared_drop_ids_by_map: Default::default(),
        };
        runtime
            .inner
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("start game should succeed");
        let mut monster = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live monster");
        monster.hp = Some(1);

        zone_state
            .lock()
            .expect("shared zone state should lock")
            .sync_map_layer(
                "0".to_string(),
                vec![monster.clone()],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        runtime.apply_shared_current_map_monsters_to_local();

        let local_hp = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.object_id == monster.object_id)
            .and_then(|entity| entity.hp);
        assert_eq!(local_hp, Some(1));
    }

    #[test]
    fn shared_in_process_runtime_broadcasts_shared_entity_movement_packets() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut owner = shared_session_runtime(zone_state.clone());
        let mut observer = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut owner);
        start_new_runtime(&mut observer, "observer-entity-move", "Blade");
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .sync_map_layer(
                "0".to_string(),
                vec![shared_monster_entity(77)],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        let movement = ObjectMovement {
            object_id: 77,
            position: Point { x: 330, y: 269 },
            direction: MirDirection::Right,
        };
        owner.apply_shared_entity_packets_to_current_map(&[ServerPacket::ObjectWalk {
            movement: movement.clone(),
        }]);
        owner.dispatch_shared_entity_observer_packets(&[ServerPacket::ObjectWalk {
            movement: movement.clone(),
        }]);

        let observer_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 312,
            }))
            .expect("observer keepalive should execute");

        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement: observed }
                if observed.object_id == movement.object_id
                    && observed.position == movement.position
                    && observed.direction == movement.direction
        )));
    }

    #[test]
    fn shared_in_process_runtime_broadcasts_shared_entity_action_packets() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut owner = shared_session_runtime(zone_state.clone());
        let mut observer = shared_session_runtime(zone_state.clone());
        let mut far_observer = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut owner);
        start_new_runtime(&mut observer, "observer-entity-action", "Blade");
        start_new_runtime(&mut far_observer, "observer-entity-action-far", "Far");
        far_observer
            .inner
            .force_authoritative_player_transform(Point { x: 380, y: 270 }, MirDirection::Down);
        let _ = far_observer.sync_zone_snapshot();
        {
            let far_key = far_observer
                .current_presence_key()
                .expect("far observer should have presence");
            let mut state = zone_state.lock().expect("shared zone state should lock");
            state.update_player_transform(&far_key, Point { x: 380, y: 270 }, MirDirection::Down);
            let session_id = SharedInProcessZoneState::zone_session_id_for_key(&far_key);
            let mut join = far_observer
                .inner
                .active_zone_join_snapshot(session_id.as_str().to_string())
                .expect("far observer should produce join snapshot");
            join.object_id = state
                .players
                .get(&far_key)
                .expect("far observer should have shared presence")
                .zone_object_id;
            join.position = Point { x: 380, y: 270 };
            let outbounds = state.zone_manager.join(join);
            let _ = state.dispatch_zone_outbounds(outbounds, Some(&far_key));
        }
        let owner_local_object_id = owner
            .local_self_object_id()
            .expect("owner should have a local self object id");
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .sync_map_layer(
                "0".to_string(),
                vec![shared_monster_entity(77)],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        let owner_zone_object_id = {
            let state = zone_state.lock().expect("shared zone state should lock");
            let owner_key = owner
                .current_presence_key()
                .expect("owner should have a presence key");
            state
                .players
                .get(&owner_key)
                .expect("owner should have shared presence")
                .zone_object_id
        };
        let attack_packet = ServerPacket::ObjectAttack {
            info: mir2_protocol::ObjectAttackInfo {
                object_id: 77,
                location: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        let struck_packet = ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: owner_local_object_id,
                attacker_id: 77,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Down,
            },
        };
        let health_packet = ServerPacket::ObjectHealth {
            info: mir2_protocol::ObjectHealthInfo {
                object_id: owner_local_object_id,
                percent: 85,
                expire: 0,
            },
        };
        let poison_packet = ServerPacket::ObjectPoisoned {
            object_id: owner_local_object_id,
            poison: 2,
        };
        let buff_packet = ServerPacket::AddBuff {
            buff: mir2_protocol::ClientBuff {
                buff_type: 3,
                visible: true,
                object_id: owner_local_object_id,
                expire_time: 1_000,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        };

        owner.dispatch_shared_entity_observer_packets(&[
            attack_packet.clone(),
            struck_packet.clone(),
            health_packet,
            poison_packet,
            buff_packet,
        ]);
        let observer_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 313,
            }))
            .expect("observer keepalive should execute");

        assert!(observer_packets
            .iter()
            .any(|packet| packet == &attack_packet));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == owner_zone_object_id && info.attacker_id == 77
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == owner_zone_object_id && info.percent == 85
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == owner_zone_object_id && *poison == 2
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == owner_zone_object_id && buff.buff_type == 3
        )));
        assert!(!observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == owner_local_object_id
        )));
        assert!(!observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == owner_local_object_id
        )));
        assert!(!observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, .. } if *object_id == owner_local_object_id
        )));
        assert!(!observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::AddBuff { buff } if buff.object_id == owner_local_object_id
        )));

        let far_packets = far_observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 314,
            }))
            .expect("far observer keepalive should execute");
        assert!(!far_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id == 77
        )));
        assert!(!far_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.attacker_id == 77
        )));
    }

    #[test]
    fn shared_in_process_registry_routes_leave_through_shared_zone() {
        let (mut first, mut second) = started_shared_zone_sessions();

        first.handle_packet(ClientPacket::LogOut);
        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 105 });

        assert!(observer_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectRemove { .. })));
        assert!(!second.world_snapshot().entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Scout"
        }));
    }

    #[test]
    fn shared_in_process_registry_removes_logged_out_remote_players() {
        let (first, mut second) = started_shared_zone_sessions();

        second.handle_packet(ClientPacket::LogOut);
        let first_snapshot = first.world_snapshot();

        assert!(!first_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Blade"
        }));
    }

    #[test]
    fn shared_in_process_runtime_removes_owner_generated_entities_on_disconnect() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut owner = shared_session_runtime(zone_state.clone());
        let mut observer = shared_session_runtime(zone_state);
        start_demo_runtime(&mut owner);
        start_new_runtime(&mut observer, "observer", "Blade");
        let owner_local_id = owner
            .local_self_object_id()
            .expect("owner should have a local object id");
        let hero_packet = ServerPacket::ObjectHero {
            info: shared_object_player_info(901, "ScoutHero", 331, 270),
            owner_name: "Scout".to_string(),
        };

        owner.apply_shared_entity_packets_to_current_map(std::slice::from_ref(&hero_packet));
        owner.dispatch_zone_observer_packets(owner_local_id, std::slice::from_ref(&hero_packet));
        let seen_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 210,
            }))
            .expect("observer keepalive should execute");
        assert!(seen_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHero { info, owner_name }
                if info.object_id == 901 && owner_name == "Scout"
        )));

        owner
            .execute(WorldCommand::ClientPacket(ClientPacket::Disconnect))
            .expect("owner disconnect should execute");
        let remove_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 211,
            }))
            .expect("observer keepalive should execute");

        assert!(remove_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 901
        )));
    }

    #[test]
    fn shared_in_process_runtime_rebases_owned_summon_master_before_disconnect_cleanup() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut owner = shared_session_runtime(zone_state.clone());
        let mut observer = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut owner);
        start_new_runtime(&mut observer, "observer-summon", "Blade");
        let owner_local_id = owner
            .local_self_object_id()
            .expect("owner should have a local object id");
        let owner_zone_object_id = {
            let state = zone_state.lock().expect("shared zone state should lock");
            let owner_key = owner
                .current_presence_key()
                .expect("owner should have a shared presence key");
            state
                .players
                .get(&owner_key)
                .expect("owner should be registered in the shared zone")
                .zone_object_id
        };
        assert_ne!(owner_local_id, owner_zone_object_id);
        let summon_packet = ServerPacket::ObjectMonster {
            info: shared_monster_info(902, owner_local_id),
        };

        owner.apply_shared_entity_packets_to_current_map(std::slice::from_ref(&summon_packet));
        owner.dispatch_zone_observer_packets(owner_local_id, std::slice::from_ref(&summon_packet));
        let seen_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 212,
            }))
            .expect("observer keepalive should execute");
        assert!(seen_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.object_id == 902 && info.master_object_id == owner_zone_object_id
        )));
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_entity("0", 902)
                .and_then(|entity| entity.owner_name),
            Some("Scout".to_string())
        );

        owner
            .execute(WorldCommand::ClientPacket(ClientPacket::Disconnect))
            .expect("owner disconnect should execute");
        let remove_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 213,
            }))
            .expect("observer keepalive should execute");

        assert!(remove_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 902
        )));
    }

    #[test]
    fn shared_in_process_runtime_removes_owned_summon_when_owner_changes_map() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut owner = shared_session_runtime(zone_state.clone());
        let mut observer = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut owner);
        start_new_runtime(&mut observer, "observer-map-change", "Blade");
        let owner_local_id = owner
            .local_self_object_id()
            .expect("owner should have a local object id");
        let summon_packet = ServerPacket::ObjectMonster {
            info: shared_monster_info(903, owner_local_id),
        };

        owner.apply_shared_entity_packets_to_current_map(std::slice::from_ref(&summon_packet));
        owner.dispatch_zone_observer_packets(owner_local_id, std::slice::from_ref(&summon_packet));
        let seen_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 214,
            }))
            .expect("observer keepalive should execute");
        assert!(seen_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info } if info.object_id == 903
        )));

        owner
            .execute(WorldCommand::TransferMap {
                key: "crystal:1:315:82".to_string(),
            })
            .expect("owner transfer should execute");
        assert_eq!(
            owner.inner.world_snapshot().map_file_name.as_deref(),
            Some("1")
        );
        let remove_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 215,
            }))
            .expect("observer keepalive should execute");

        assert!(remove_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 903
        )));
        let map = zone_state
            .lock()
            .expect("shared zone state should lock")
            .map_layer(Some("0"))
            .expect("old shared map should exist");
        assert!(!map.entities.contains_key(&903));
        assert!(map.removed_entity_ids.contains(&903));
    }

    #[test]
    fn shared_in_process_registry_surfaces_shared_npcs_for_sparse_sessions() {
        let registry = ZoneRegistry::in_process();
        let mut first = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut first);
        let shared_npc = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::Npc)
            .expect("default session should have a visible NPC");

        let mut sparse_config = GatewayConfig::default();
        sparse_config.visible_npcs.clear();
        let mut second = GatewaySession::new_with_zone_registry(sparse_config, &registry);
        start_new_character(&mut second, "second", "Blade");
        let second_snapshot = second.world_snapshot();

        assert!(second_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Npc && entity.name == shared_npc.name
        }));

        second.transfer_map(&format!(
            "crystal:0:{}:{}",
            shared_npc.x.saturating_sub(1),
            shared_npc.y
        ));
        let packets = second.interact(shared_npc.object_id);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == shared_npc.object_id
        )));
        assert_eq!(
            second
                .world_snapshot()
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(shared_npc.object_id)
        );
    }

    #[test]
    fn shared_in_process_registry_callnpc_shared_guide_starts_sparse_session_quest() {
        let registry = ZoneRegistry::in_process();
        let mut first = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut first);
        let shared_guide = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == mir2_simulation::WorldEntityKind::Npc
                    && entity.quest_ids.contains(&1001)
            })
            .expect("default session should expose a shared quest NPC");

        let mut sparse_config = GatewayConfig::default();
        sparse_config.visible_npcs.clear();
        let mut second = GatewaySession::new_with_zone_registry(sparse_config, &registry);
        start_new_character(&mut second, "second", "Blade");
        second.transfer_map(&format!(
            "crystal:0:{}:{}",
            shared_guide.x.saturating_sub(1),
            shared_guide.y
        ));

        let packets = second.handle_packet(ClientPacket::CallNpc {
            object_id: shared_guide.object_id,
            key: "@Main".to_string(),
        });
        let snapshot = second.world_snapshot();

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == shared_guide.object_id
        )));
        assert_eq!(
            snapshot
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(shared_guide.object_id)
        );
        assert!(
            snapshot.quest_log.iter().any(|quest| {
                shared_guide.quest_ids.contains(&quest.quest_id)
                    && quest.stage == QuestStage::InProgress
            }),
            "shared guide CallNpc should start a quest; quests: {:?}",
            snapshot.quest_log
        );
    }

    #[test]
    fn shared_in_process_registry_syncs_npc_saved_values_between_sessions() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut first, "npc-value-owner", "Scout");
        start_new_runtime(&mut second, "npc-value-reader", "Blade");

        let saved = SharedNpcSavedValue {
            file_name: "quests\\flags.txt".to_string(),
            group: "profile".to_string(),
            key: "answer".to_string(),
            value: "Scout".to_string(),
        };
        first.inner.apply_shared_npc_saved_values(&[saved.clone()]);
        first.publish_shared_npc_saved_values_from_local();

        assert!(second.inner.shared_npc_saved_values().is_empty());
        second.apply_shared_npc_saved_values_to_local();
        assert_eq!(second.inner.shared_npc_saved_values(), vec![saved.clone()]);

        second
            .inner
            .apply_shared_npc_saved_values(&[SharedNpcSavedValue {
                value: "Blade".to_string(),
                ..saved.clone()
            }]);
        second.publish_shared_npc_saved_values_from_local();
        first.apply_shared_npc_saved_values_to_local();

        assert_eq!(
            first
                .inner
                .shared_npc_saved_values()
                .into_iter()
                .find(|value| {
                    value.file_name.eq_ignore_ascii_case(&saved.file_name)
                        && value.group.eq_ignore_ascii_case(&saved.group)
                        && value.key.eq_ignore_ascii_case(&saved.key)
                })
                .map(|value| value.value),
            Some("Blade".to_string())
        );
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_saved_values()
                .len(),
            1
        );
    }

    #[test]
    fn shared_in_process_registry_syncs_npc_random_seed_between_sessions() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut first, "npc-random-owner", "Scout");
        start_new_runtime(&mut second, "npc-random-reader", "Blade");

        first.inner.apply_shared_npc_random_seed(42);
        first.publish_shared_npc_random_seed_from_local();

        assert_eq!(second.inner.shared_npc_random_seed(), 0);
        second.apply_shared_npc_random_seed_to_local();
        assert_eq!(second.inner.shared_npc_random_seed(), 42);

        second.inner.apply_shared_npc_random_seed(9001);
        second.publish_shared_npc_random_seed_from_local();
        first.apply_shared_npc_random_seed_to_local();

        assert_eq!(first.inner.shared_npc_random_seed(), 9001);
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .shared_npc_random_seed(),
            Some(9001)
        );
    }

    #[test]
    fn shared_in_process_registry_relays_share_quest_to_online_group_member() {
        let (mut first, mut second) = started_shared_zone_sessions();

        first.handle_packet(ClientPacket::AddMember {
            name: "Blade".to_string(),
        });
        let owner_packets = first.handle_packet(ClientPacket::ShareQuest { quest_index: 1001 });
        let receiver_packets = second.tick();

        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ShareQuest {
                quest_index: 1001,
                sharer_name,
            } if sharer_name == "Scout"
        )));
        assert!(receiver_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ShareQuest {
                quest_index: 1001,
                sharer_name,
            } if sharer_name == "Scout"
        )));
    }

    #[test]
    fn shared_in_process_registry_surfaces_shared_ground_drops() {
        let (mut first, second) = started_shared_zone_sessions();
        let item_key = "dagger".to_string();
        assert!(has_inventory_key(&first, &item_key));

        first.drop_item(&item_key);
        let shared_drop_name = first
            .world_snapshot()
            .ground_drops
            .first()
            .map(|drop| drop.name.clone())
            .expect("dropped inventory item should appear on the ground");
        let second_snapshot = second.world_snapshot();

        assert!(second_snapshot
            .ground_drops
            .iter()
            .any(|drop| drop.name == shared_drop_name));
    }

    #[test]
    fn shared_in_process_registry_broadcasts_player_drop_gold_spawn() {
        let (mut first, mut second) = started_shared_zone_sessions();

        let owner_packets = first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let gold_location = owner_packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::ObjectGold { info } => Some(info.location.clone()),
                _ => None,
            })
            .expect("owner should receive ObjectGold for dropped gold");
        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 106 });

        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectGold { info } if info.gold == 100 && info.location == gold_location
        )));
    }

    #[test]
    fn shared_in_process_registry_removes_remote_picked_up_shared_drop() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let item_key = first
            .world_snapshot()
            .inventory_items
            .first()
            .map(|item| item.key.clone())
            .expect("demo character should have a seeded inventory item");
        first.drop_item(&item_key);
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared ground drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.pick_up(shared_drop.object_id);

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        let second_snapshot = second.world_snapshot();
        assert!(second_snapshot
            .inventory_items
            .iter()
            .chain(second_snapshot.belt_items.iter())
            .any(|item| item.name == shared_drop.name));
        assert!(!second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));

        first.tick();

        assert!(!second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_runtime_claims_shared_drop_through_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "zone-claim-second", "Blade");

        first
            .execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
                amount: 100,
            }))
            .expect("owner drop gold should execute");
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second runtime should see the shared gold drop");
        assert!(zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .is_some_and(|zone| zone.has_ground_drop(shared_drop.object_id)));

        second
            .execute(WorldCommand::TransferMap {
                key: format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y),
            })
            .expect("second transfer should execute");
        let packets = second
            .execute(WorldCommand::PickUp {
                object_id: shared_drop.object_id,
            })
            .expect("second shared pickup should execute");

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        let state = zone_state.lock().expect("shared zone state should lock");
        assert!(state
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .is_some_and(|zone| !zone.has_ground_drop(shared_drop.object_id)));
        assert!(!state
            .maps
            .get("0")
            .expect("shared map should exist")
            .ground_drops
            .contains_key(&shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_removes_packet_picked_up_shared_drop() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let item_key = first
            .world_snapshot()
            .inventory_items
            .first()
            .map(|item| item.key.clone())
            .expect("demo character should have a seeded inventory item");
        first.drop_item(&item_key);
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared ground drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.handle_packet(ClientPacket::PickUp);

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        let second_snapshot = second.world_snapshot();
        assert!(second_snapshot
            .inventory_items
            .iter()
            .chain(second_snapshot.belt_items.iter())
            .any(|item| item.name == shared_drop.name));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_runtime_rolls_back_shared_gold_claim_when_gold_cap_blocks_commit() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "zone-claim-goldcap", "Blade");
        let session_id = runtime
            .current_zone_session_id()
            .expect("runtime should have joined a shared zone");
        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let seed_gold = GroundDropSnapshot {
            object_id: 8800,
            name: "Seed Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: self_entity.x,
            y: self_entity.y,
            quantity: 1,
            source_monster: "rollback-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 1 },
        };
        let seed_packets = runtime.inner.apply_shared_ground_drop_pickup(&seed_gold);
        assert!(seed_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 1 })));

        let shared_drop = GroundDropSnapshot {
            object_id: 8801,
            name: "Rollback Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: self_entity.x,
            y: self_entity.y,
            quantity: u32::MAX,
            source_monster: "rollback-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: u32::MAX },
        };
        runtime.dispatch_zone_player_command(
            ZoneCommand::SyncGroundDrops {
                session_id,
                drops: vec![shared_drop.clone()],
                now_ms: shared_gateway_now_ms(),
            },
            false,
        );

        let packets = runtime
            .execute(WorldCommand::PickUp {
                object_id: shared_drop.object_id,
            })
            .expect("shared gold pickup should execute");

        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { .. })));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectGold { info } if info.object_id == shared_drop.object_id
        )));

        let state = zone_state.lock().expect("shared zone state should lock");
        assert!(state
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .is_some_and(|zone| zone.has_ground_drop(shared_drop.object_id)));
        assert!(state
            .maps
            .get("0")
            .expect("shared map should exist")
            .ground_drops
            .contains_key(&shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_gains_remote_picked_up_shared_gold() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let starting_gold = second.world_snapshot().gold;

        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared gold drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.pick_up(shared_drop.object_id);

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        assert_eq!(second.world_snapshot().gold, starting_gold + 100);
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_runtime_expires_shared_drop_on_keepalive() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "drop-expire-observer", "Blade");

        first
            .execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
                amount: 100,
            }))
            .expect("owner drop gold should execute");
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second runtime should see the shared gold drop");
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .maps
            .get_mut("0")
            .expect("shared map should exist")
            .drop_expires_at_ms
            .insert(shared_drop.object_id, 0);

        let owner_packets = first
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 310,
            }))
            .expect("owner keepalive should execute");
        let observer_packets = second
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 311,
            }))
            .expect("observer keepalive should execute");

        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
        assert!(!second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_intelligent_creature_picks_up_remote_shared_gold() {
        let (mut first, mut second) = started_shared_zone_sessions();
        second.handle_packet(ClientPacket::UpdateIntelligentCreature {
            creature: shared_pickup_creature(),
            summon_me: true,
            unsummon_me: false,
            release_me: false,
        });
        let starting_gold = second.world_snapshot().gold;

        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared gold drop");

        let packets = second.handle_packet(ClientPacket::IntelligentCreaturePickup {
            mouse_mode: true,
            location: Point {
                x: shared_drop.x,
                y: shared_drop.y,
            },
        });

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::IntelligentCreaturePickup { object_id }
                if *object_id == shared_drop.object_id
        )));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        assert_eq!(second.world_snapshot().gold, starting_gold + 100);
        assert!(!second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));

        let observer_packets = first.handle_packet(ClientPacket::KeepAlive { time: 107 });
        assert!(observer_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::IntelligentCreaturePickup { object_id }
            if *object_id == shared_drop.object_id
        )));
    }

    #[test]
    fn shared_in_process_registry_intelligent_creature_auto_picks_remote_shared_gold() {
        let (mut first, mut second) = started_shared_zone_sessions();
        second.handle_packet(ClientPacket::UpdateIntelligentCreature {
            creature: shared_pickup_creature(),
            summon_me: true,
            unsummon_me: false,
            release_me: false,
        });
        let starting_gold = second.world_snapshot().gold;

        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared gold drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.tick();

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::IntelligentCreaturePickup { object_id }
                if *object_id == shared_drop.object_id
        )));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        assert_eq!(second.world_snapshot().gold, starting_gold + 100);
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));

        let observer_packets = first.handle_packet(ClientPacket::KeepAlive { time: 108 });
        assert!(observer_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::IntelligentCreaturePickup { object_id }
            if *object_id == shared_drop.object_id
        )));
    }

    #[test]
    fn shared_in_process_registry_intelligent_creature_filter_blocks_remote_shared_item() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let mut creature = shared_pickup_creature();
        creature.filter.pet_pickup_all = false;
        creature.filter.pet_pickup_gold = true;
        creature.filter.pet_pickup_others = false;
        creature.filter.pet_pickup_weapons = false;
        creature.creature_rules.auto_pickup_enabled = false;
        second.handle_packet(ClientPacket::UpdateIntelligentCreature {
            creature,
            summon_me: true,
            unsummon_me: false,
            release_me: false,
        });
        let item_key = first
            .world_snapshot()
            .inventory_items
            .first()
            .map(|item| item.key.clone())
            .expect("demo character should have a seeded inventory item");

        first.drop_item(&item_key);
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared item drop");
        let packets = second.handle_packet(ClientPacket::IntelligentCreaturePickup {
            mouse_mode: true,
            location: Point {
                x: shared_drop.x,
                y: shared_drop.y,
            },
        });

        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::IntelligentCreaturePickup { .. } | ServerPacket::GainedItem { .. }
        )));
        assert!(second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
        assert!(first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_uses_adjacent_remote_player_for_item_rental_request() {
        let (mut first, _second) = started_shared_zone_sessions();

        let packets = first.handle_packet(ClientPacket::ItemRentalRequest);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRentalRequest {
                name,
                renting: false
            } if name == "Blade"
        )));
    }

    #[test]
    fn shared_in_process_registry_commits_two_sided_item_rental_delivery() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 10 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see rental funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);

        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;
        let first_dagger_slot = inventory_slot_for_key(&first, "dagger");

        let request_packets = first.handle_packet(ClientPacket::ItemRentalRequest);
        assert!(request_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRentalRequest {
                name,
                renting: false
            } if name == "Blade"
        )));

        let invite_packets = second.handle_packet(ClientPacket::KeepAlive { time: 10 });
        assert!(invite_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRentalRequest {
                name,
                renting: true
            } if name == "Scout"
        )));

        let fee_packets = second.handle_packet(ClientPacket::ItemRentalFee { amount: 10 });
        assert!(fee_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 10 })));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 10);
        assert!(second
            .handle_packet(ClientPacket::ItemRentalLockFee)
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::ItemRentalLock {
                    success: true,
                    gold_locked: true,
                    item_locked: false
                }
            )));

        first.handle_packet(ClientPacket::ItemRentalPeriod { days: 3 });
        first.handle_packet(ClientPacket::DepositRentalItem {
            from: first_dagger_slot,
            to: 0,
        });
        assert!(first
            .handle_packet(ClientPacket::ItemRentalLockItem)
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::ItemRentalLock {
                    success: true,
                    gold_locked: false,
                    item_locked: true
                }
            )));

        let lender_confirm = first.handle_packet(ClientPacket::ConfirmItemRental);
        assert!(lender_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 10 })));
        assert!(lender_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ConfirmItemRental)));
        assert!(lender_confirm.iter().any(|packet| matches!(
            packet,
            ServerPacket::GetRentedItems { rented_items }
                if rented_items.len() == 1
                    && rented_items[0].item_name == "Dagger"
                    && rented_items[0].renting_player_name == "Blade"
        )));
        assert_eq!(first.world_snapshot().gold, first_starting_gold + 10);
        assert!(!has_inventory_key(&first, "dagger"));

        let borrower_delivery = second.handle_packet(ClientPacket::KeepAlive { time: 11 });
        assert!(borrower_delivery.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item }
                if item
                    .rental_information
                    .as_ref()
                    .is_some_and(|info| info.owner_name == "Scout"
                        && info.expiry_binary_datetime != 0)
        )));
        assert!(borrower_delivery
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ConfirmItemRental)));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 10);
    }

    #[test]
    fn shared_in_process_registry_rolls_back_item_rental_when_partner_cancels() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 10 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see rental funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);

        let second_starting_gold = second.world_snapshot().gold;
        let first_dagger_slot = inventory_slot_for_key(&first, "dagger");
        first.handle_packet(ClientPacket::ItemRentalRequest);
        second.handle_packet(ClientPacket::KeepAlive { time: 20 });
        second.handle_packet(ClientPacket::ItemRentalFee { amount: 10 });
        second.handle_packet(ClientPacket::ItemRentalLockFee);
        first.handle_packet(ClientPacket::DepositRentalItem {
            from: first_dagger_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::ItemRentalLockItem);
        assert!(!has_inventory_key(&first, "dagger"));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 10);

        let cancel_packets = second.handle_packet(ClientPacket::CancelItemRental);
        assert!(cancel_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CancelItemRental)));
        assert_eq!(second.world_snapshot().gold, second_starting_gold);

        let lender_cancel = first.handle_packet(ClientPacket::KeepAlive { time: 21 });
        assert!(lender_cancel
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CancelItemRental)));
        assert!(has_inventory_key(&first, "dagger"));
    }

    #[test]
    fn shared_in_process_registry_uses_adjacent_remote_player_for_trade_request() {
        let (mut first, _second) = started_shared_zone_sessions();

        let packets = first.handle_packet(ClientPacket::TradeRequest);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::TradeRequest { name } if name == "Blade"
        )));
        assert!(first
            .handle_packet(ClientPacket::TradeReply {
                accept_invite: true,
            })
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::TradeAccept { name } if name == "Blade"
            )));
    }

    #[test]
    fn shared_in_process_registry_commits_two_sided_trade_delivery() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);

        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        let first_confirm = first.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert!(first_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 30 })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::TradeGold { amount: 40 });
        let second_confirm = second.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert!(second_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 40 })));
        assert!(second_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(second_confirm.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 40 + 30);

        let first_delivery = first.handle_packet(ClientPacket::KeepAlive { time: 1 });
        assert!(first_delivery
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 40 })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30 + 40);
    }

    #[test]
    fn shared_in_process_registry_rolls_back_pending_trade_when_partner_cancels() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let first_starting_gold = first.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::TradeCancel);
        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 2 });

        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(rollback.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: false })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    #[test]
    fn shared_in_process_registry_rolls_back_pending_trade_when_partner_disconnects() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let first_starting_gold = first.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::LogOut);
        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 3 });

        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(rollback.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    #[test]
    fn shared_in_process_registry_rolls_back_two_sided_trade_when_receiver_is_full() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);
        fill_gateway_bag(&mut second);

        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::TradeGold { amount: 40 });
        let failed_confirm = second.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert!(failed_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 40 })));
        assert!(failed_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 40 })));
        assert!(failed_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: false })));
        assert!(failed_confirm
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedItem { .. })));
        assert_eq!(second.world_snapshot().gold, second_starting_gold);

        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 4 });
        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(rollback.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    fn inventory_slot_for_key(session: &GatewaySession, key: &str) -> i32 {
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .find(|item| item.key == key)
            .map(|item| i32::from(item.slot))
            .unwrap_or_else(|| panic!("{key} should exist in inventory"))
    }

    fn has_inventory_key(session: &GatewaySession, key: &str) -> bool {
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .any(|item| item.key == key)
    }

    fn fill_gateway_bag(session: &mut GatewaySession) {
        for index in 0..100 {
            if session.world_snapshot().free_bag_slots == 0 {
                return;
            }
            session.stage5_command("qa.giveItem", vec![format!("trade-filler-{index}")]);
        }
        assert_eq!(session.world_snapshot().free_bag_slots, 0);
    }

    fn started_shared_zone_sessions() -> (GatewaySession, GatewaySession) {
        let registry = ZoneRegistry::in_process();
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut second = GatewaySession::new_with_zone_registry(config, &registry);

        start_demo_character(&mut first);
        start_new_character(&mut second, "second", "Blade");
        (first, second)
    }

    fn shared_session_runtime(
        zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    ) -> SharedInProcessZoneSessionRuntime {
        shared_session_runtime_with_services(
            zone_state,
            Arc::new(InProcessAccountInventoryService::new()),
            Arc::new(InProcessNpcWorldService),
        )
    }

    fn shared_session_runtime_with_account_inventory_service(
        zone_state: Arc<Mutex<SharedInProcessZoneState>>,
        account_inventory_service: SharedAccountInventoryServiceHandle,
    ) -> SharedInProcessZoneSessionRuntime {
        shared_session_runtime_with_services(
            zone_state,
            account_inventory_service,
            Arc::new(InProcessNpcWorldService),
        )
    }

    fn shared_session_runtime_with_services(
        zone_state: Arc<Mutex<SharedInProcessZoneState>>,
        account_inventory_service: SharedAccountInventoryServiceHandle,
        npc_world_service: SharedNpcWorldServiceHandle,
    ) -> SharedInProcessZoneSessionRuntime {
        SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(GatewayConfig::default()),
            zone_state,
            account_inventory_service,
            npc_world_service,
            presence_key: None,
            zone_move_seq: 0,
            shared_skill_item_request_seq: 0,
            force_next_zone_transform_sync: false,
            cached_map_file_name: None,
            cached_local_self_object_id: None,
            cached_map_transfers: Vec::new(),
            last_shared_entity_ids_by_map: Default::default(),
            last_shared_drop_ids_by_map: Default::default(),
        }
    }

    fn start_demo_character(session: &mut GatewaySession) {
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    }

    fn start_demo_runtime(runtime: &mut SharedInProcessZoneSessionRuntime) {
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("demo login should execute");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("demo start should execute");
    }

    fn start_new_character(session: &mut GatewaySession, account_id: &str, name: &str) {
        session.handle_packet(ClientPacket::NewAccount {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        });
        session.handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
        });
        let character_index = session
            .handle_packet(ClientPacket::NewCharacter {
                name: name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            })
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("new character should return an index");
        session.handle_packet(ClientPacket::StartGame { character_index });
    }

    fn start_new_runtime(
        runtime: &mut SharedInProcessZoneSessionRuntime,
        account_id: &str,
        name: &str,
    ) {
        start_new_runtime_with_class(runtime, account_id, name, MirClass::Warrior);
    }

    fn start_new_runtime_with_class(
        runtime: &mut SharedInProcessZoneSessionRuntime,
        account_id: &str,
        name: &str,
        class: MirClass,
    ) {
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::NewAccount {
                account_id: account_id.to_string(),
                password: account_id.to_string(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            }))
            .expect("new account should execute");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: account_id.to_string(),
                password: account_id.to_string(),
            }))
            .expect("new runtime login should execute");
        let character_index = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
                name: name.to_string(),
                gender: MirGender::Male,
                class,
            }))
            .expect("new runtime character should execute")
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("new runtime character should return an index");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index,
            }))
            .expect("new runtime start should execute");
    }

    fn shared_monster_entity(object_id: u32) -> WorldEntitySnapshot {
        WorldEntitySnapshot {
            object_id,
            kind: WorldEntityKind::Monster,
            name: "Deer".to_string(),
            owner_name: None,
            x: 329,
            y: 269,
            direction: MirDirection::Down,
            class: None,
            gender: None,
            level: None,
            hp: Some(12),
            max_hp: Some(12),
            name_colour_argb: -1,
            dead: false,
            disposition: WorldEntityDisposition::Neutral,
            sprite: None,
            quest_ids: Vec::new(),
        }
    }

    fn shared_monster_info(object_id: u32, master_object_id: u32) -> MonsterInfo {
        MonsterInfo {
            object_id,
            name: "Shinsu".to_string(),
            name_colour_argb: -1,
            location: Point { x: 331, y: 270 },
            image: 33,
            direction: MirDirection::Down,
            effect: 0,
            ai: 6,
            light: 0,
            dead: false,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id,
            rarity: 0,
            buffs: Vec::new(),
        }
    }

    fn shared_object_player_info(object_id: u32, name: &str, x: i32, y: i32) -> ObjectPlayerInfo {
        ObjectPlayerInfo {
            object_id,
            name: name.to_string(),
            guild_name: String::new(),
            guild_rank_name: String::new(),
            name_colour_argb: -1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            location: Point { x, y },
            direction: MirDirection::Down,
            hair: 0,
            light: 0,
            weapon: -1,
            weapon_effect: 0,
            armour: -1,
            poison: 0,
            dead: false,
            hidden: false,
            effect: 0,
            wing_effect: 0,
            extra: false,
            mount_type: -1,
            riding_mount: false,
            fishing: false,
            transform_type: 0,
            element_orb_effect: 0,
            element_orb_level: 0,
            element_orb_max: 0,
            buffs: Vec::new(),
            level_effects: 0,
        }
    }

    fn shared_picker_entity(object_id: u32, x: i32, y: i32) -> WorldEntitySnapshot {
        WorldEntitySnapshot {
            object_id,
            kind: WorldEntityKind::SelfPlayer,
            name: "Picker".to_string(),
            owner_name: None,
            x,
            y,
            direction: MirDirection::Down,
            class: Some(MirClass::Warrior),
            gender: Some(MirGender::Male),
            level: Some(7),
            hp: None,
            max_hp: None,
            name_colour_argb: -1,
            dead: false,
            disposition: WorldEntityDisposition::Friendly,
            sprite: None,
            quest_ids: Vec::new(),
        }
    }

    fn shared_gold_drop(
        object_id: u32,
        x: i32,
        y: i32,
        owner_object_id: Option<u32>,
        ownership_remaining_ticks: Option<u64>,
    ) -> GroundDropSnapshot {
        GroundDropSnapshot {
            object_id,
            name: "Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x,
            y,
            quantity: 1,
            source_monster: "Deer".to_string(),
            owner_object_id,
            ownership_remaining_ticks,
            loot: GroundDropLootSnapshot::Gold { amount: 25 },
        }
    }

    fn shared_pickup_creature() -> ClientIntelligentCreature {
        ClientIntelligentCreature {
            pet_type: 1,
            icon: 44,
            custom_name: "Buddy".to_string(),
            fullness: 1200,
            slot_index: 0,
            expire_binary_datetime: 638000000000000000,
            blackstone_time: 0,
            pet_mode: 1,
            creature_rules: IntelligentCreatureRules {
                minimal_fullness: 0,
                mouse_pickup_enabled: true,
                mouse_pickup_range: 0,
                auto_pickup_enabled: true,
                auto_pickup_range: 3,
                semi_auto_pickup_enabled: true,
                semi_auto_pickup_range: 0,
                can_produce_blackstone: true,
            },
            filter: IntelligentCreatureItemFilter {
                pet_pickup_all: false,
                pet_pickup_gold: true,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: true,
            },
            pickup_grade: 0,
            maintain_food_time: 24_000,
        }
    }
}
