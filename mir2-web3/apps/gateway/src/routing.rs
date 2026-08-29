use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{
    mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    Arc, Condvar, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_game_data::crystal_monster_by_name;
use mir2_protocol::{
    decode_server_packet, encode_server_packet, ClientPacket, MirDirection, MonsterInfo, NpcInfo,
    ObjectDiedInfo, ObjectGoldInfo, ObjectHealthInfo, ObjectItemInfo, ObjectPlayerInfo, Point,
    ServerPacket, Spell, UserLocation,
};
use mir2_simulation::{
    intelligent_creature_allows_ground_drop, world_entity_sprite_from_object_player,
    zone_ground_drop_snapshots_for_monster_at_tick, ActiveSessionIdentity, CharacterSaveRecord,
    ChatPacketPreparation, GameShopPurchaseOutcome, GroundDropClaimTicket, GroundDropLootSnapshot,
    GroundDropSnapshot, InProcessWorldRuntime, SessionId, SharedAccountInventoryTransactionKind,
    SharedAccountInventoryTransactionReceipt, SharedInventoryItemDrop, SharedItemRentalAgreement,
    SharedItemRentalDelivery, SharedItemRentalFeeOffer, SharedItemRentalItemOffer,
    SharedNpcSavedValue, SharedSkillItemConsumptionComponent, SharedTradeOffer, WorldCommand,
    WorldCommandExecution, WorldCommandOutcome, WorldEntityDisposition, WorldEntityKind,
    WorldEntitySnapshot, WorldEntitySpriteSnapshot, WorldRuntime, WorldSnapshot,
    ZoneBossRewardAudit, ZoneCommand, ZoneKey, ZoneManager, ZoneMonsterDefense,
    ZoneMonsterKillAward, ZoneMonsterSpawn, ZoneNativeMonsterSnapshot, ZoneOutbound,
    ZoneRuntimeHandle, CRYSTAL_OBJECT_DATA_RANGE,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{error::TrySendError as TokioTrySendError, Sender as TokioMpscSender};

use crate::GatewayConfig;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
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

/// One immutable teardown snapshot produced at the authoritative Zone owner.
///
/// Identity and character state must be captured by the same owner-side
/// operation. The lease records which fenced owner generation produced the
/// snapshot, so callers can reject a handoff that races preparation.
#[derive(Debug, Clone)]
pub struct PreparedZoneTeardown {
    pub(crate) owner_lease: ZoneOwnerLease,
    pub(crate) identity: ActiveSessionIdentity,
    pub(crate) checkpoint: CharacterSaveRecord,
}

impl PreparedZoneTeardown {
    fn new(
        owner_lease: ZoneOwnerLease,
        identity: ActiveSessionIdentity,
        checkpoint: CharacterSaveRecord,
    ) -> Self {
        Self {
            owner_lease,
            identity,
            checkpoint,
        }
    }

    pub fn owner_lease(&self) -> &ZoneOwnerLease {
        &self.owner_lease
    }

    pub fn identity(&self) -> &ActiveSessionIdentity {
        &self.identity
    }

    pub fn checkpoint(&self) -> &CharacterSaveRecord {
        &self.checkpoint
    }

    pub(crate) fn validate_identity_checkpoint(&self) -> Result<(), String> {
        if self.identity.account_id.is_empty()
            || self.identity.account_id.as_str() != self.identity.account_id.trim()
        {
            return Err("teardown identity requires a nonempty canonical account id".to_string());
        }
        if self.identity.character_index < 0 {
            return Err("teardown identity requires a nonnegative character index".to_string());
        }
        if self.identity.character_name.is_empty() {
            return Err("teardown identity requires a nonempty character name".to_string());
        }
        if self.checkpoint.character.index != self.identity.character_index {
            return Err(
                "teardown checkpoint character index does not match its identity".to_string(),
            );
        }
        if self.checkpoint.character.name != self.identity.character_name {
            return Err(
                "teardown checkpoint character name does not match its identity".to_string(),
            );
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneOwnerCommandMode {
    Direct,
    ProductionPlayer { authenticated: bool },
}

#[derive(Debug, Clone)]
pub struct ZoneOwnerCommandRequest {
    owner_lease: ZoneOwnerLease,
    mode: ZoneOwnerCommandMode,
    command: WorldCommand,
    source_sequence: Option<u64>,
}

impl ZoneOwnerCommandRequest {
    pub fn direct(owner_lease: ZoneOwnerLease, command: WorldCommand) -> Self {
        Self {
            owner_lease,
            mode: ZoneOwnerCommandMode::Direct,
            command,
            source_sequence: None,
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
            source_sequence: None,
        }
    }

    /// Bind an already-ordered Zone mutation sequence to external side effects.
    ///
    /// Gateway-originated requests do not choose this number. The authoritative
    /// Zone Host assigns it while holding its operation gate, before executing
    /// the command. Verified standby replay reuses the source sequence carried
    /// by the mutation batch.
    pub(crate) fn with_source_sequence(mut self, source_sequence: u64) -> Self {
        self.source_sequence = Some(source_sequence);
        self
    }

    pub fn owner_lease(&self) -> &ZoneOwnerLease {
        &self.owner_lease
    }

    pub fn mode(&self) -> ZoneOwnerCommandMode {
        self.mode
    }

    pub fn command(&self) -> &WorldCommand {
        &self.command
    }

    pub fn source_sequence(&self) -> Option<u64> {
        self.source_sequence
    }

    pub fn into_command(self) -> WorldCommand {
        self.command
    }

    pub fn into_parts(self) -> (ZoneOwnerLease, ZoneOwnerCommandMode, WorldCommand) {
        (self.owner_lease, self.mode, self.command)
    }
}

pub trait ZoneOwnerLeaseAuthority: Send + Sync {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease;

    fn refresh_owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.owner_lease(zone_id)
    }

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
    fn on_connect(&self, runtime: &ZoneRuntimeHandle) -> Result<Vec<ServerPacket>, String> {
        Ok(runtime.on_connect())
    }

    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String>;

    /// Execute one native-receipt purchase only after proving that this exact
    /// command path can return the authoritative typed transaction outcome.
    /// Generic Web/Crystal purchases intentionally continue through
    /// `execute`, including against rolling old Zone Hosts.
    fn execute_requiring_typed_game_shop_purchase_outcome(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        if !matches!(request.command(), WorldCommand::NativeGameShopPurchase(_)) {
            return Err(
                "typed GameShop outcome execution requires a native idempotent purchase"
                    .to_string(),
            );
        }
        if !self.supports_typed_game_shop_purchase_outcome(runtime) {
            return Err(
                "typed GameShop purchase outcome capability is unavailable before execution"
                    .to_string(),
            );
        }
        self.execute(runtime, request)
    }

    fn supports_typed_game_shop_purchase_outcome(&self, runtime: &ZoneRuntimeHandle) -> bool {
        runtime.supports_typed_game_shop_purchase_outcome()
    }

    fn world_snapshot(&self, runtime: &ZoneRuntimeHandle) -> Result<WorldSnapshot, String> {
        Ok(runtime.world_snapshot())
    }

    fn active_identity(
        &self,
        runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<ActiveSessionIdentity>, String> {
        Ok(runtime.active_identity())
    }

    fn active_character_checkpoint(
        &self,
        runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<CharacterSaveRecord>, String> {
        Ok(runtime.active_character_checkpoint())
    }

    fn restore_active_character_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        runtime.restore_active_character_checkpoint(checkpoint)
    }

    fn prepare_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        prepare_zone_teardown_checkpoint(runtime, owner_lease)
    }

    fn persist_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        persist_zone_teardown_checkpoint(runtime, prepared)
    }

    fn release_teardown_fence(&self, runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        release_zone_teardown_fence(runtime)
    }

    fn save_active_character(&self, runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        runtime.save_active_character()
    }

    fn refresh_active_external_mail(
        &self,
        runtime: &mut ZoneRuntimeHandle,
    ) -> Result<bool, String> {
        Ok(runtime.refresh_active_external_mail())
    }

    fn close_session(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        _owner_lease: &ZoneOwnerLease,
    ) -> Result<(), String> {
        Ok(())
    }

    fn register_live_outbound(
        &self,
        runtime: &ZoneRuntimeHandle,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        let Some(ingress) = shared_zone_movement_ingress(runtime) else {
            return Ok(None);
        };
        ingress.register_live_outbound(sender).map(|registration| {
            registration
                .map(|registration| Box::new(registration) as Box<dyn ZoneLiveOutboundRegistration>)
        })
    }
}

pub type SharedZoneOwnerCommandClient = Arc<dyn ZoneOwnerCommandClient>;

pub trait ZoneOwnerRpcTransport: fmt::Debug + Send + Sync {
    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        Err("zone owner RPC transport does not implement on_connect".to_string())
    }

    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String>;

    fn execute_requiring_typed_game_shop_purchase_outcome(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        if !matches!(request.command(), WorldCommand::NativeGameShopPurchase(_)) {
            return Err(
                "typed GameShop outcome execution requires a native idempotent purchase"
                    .to_string(),
            );
        }
        if !self.supports_typed_game_shop_purchase_outcome() {
            return Err(
                "typed GameShop purchase outcome capability is unavailable before execution"
                    .to_string(),
            );
        }
        self.execute(request)
    }

    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        false
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String>;

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String>;

    fn active_character_checkpoint(&self) -> Result<Option<CharacterSaveRecord>, String> {
        Err("zone owner RPC transport does not implement active_character_checkpoint".to_string())
    }

    fn restore_active_character_checkpoint(
        &self,
        _checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        Err(
            "zone owner RPC transport does not implement restore_active_character_checkpoint"
                .to_string(),
        )
    }

    fn prepare_teardown_checkpoint(
        &self,
        _owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        Err("zone owner RPC transport does not implement prepare_teardown_checkpoint".to_string())
    }

    fn persist_teardown_checkpoint(&self, _prepared: &PreparedZoneTeardown) -> Result<(), String> {
        Err("zone owner RPC transport does not implement persist_teardown_checkpoint".to_string())
    }

    fn release_teardown_fence(&self) -> Result<(), String> {
        Err("zone owner RPC transport does not implement release_teardown_fence".to_string())
    }

    fn save_active_character(&self) -> Result<(), String>;

    fn refresh_active_external_mail(&self) -> Result<bool, String>;

    fn close_session(&self, _owner_lease: &ZoneOwnerLease) -> Result<(), String> {
        Ok(())
    }

    fn register_live_outbound(
        &self,
        _sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        Ok(None)
    }
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
    fn on_connect(&self, _runtime: &ZoneRuntimeHandle) -> Result<Vec<ServerPacket>, String> {
        self.transport.on_connect()
    }

    fn execute(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.transport.execute(request)
    }

    fn execute_requiring_typed_game_shop_purchase_outcome(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.transport
            .execute_requiring_typed_game_shop_purchase_outcome(request)
    }

    fn supports_typed_game_shop_purchase_outcome(&self, _runtime: &ZoneRuntimeHandle) -> bool {
        self.transport.supports_typed_game_shop_purchase_outcome()
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

    fn active_character_checkpoint(
        &self,
        _runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<CharacterSaveRecord>, String> {
        self.transport.active_character_checkpoint()
    }

    fn restore_active_character_checkpoint(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.transport
            .restore_active_character_checkpoint(checkpoint)
    }

    fn prepare_teardown_checkpoint(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        self.transport.prepare_teardown_checkpoint(owner_lease)
    }

    fn persist_teardown_checkpoint(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        self.transport.persist_teardown_checkpoint(prepared)
    }

    fn release_teardown_fence(&self, _runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        self.transport.release_teardown_fence()
    }

    fn save_active_character(&self, _runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        self.transport.save_active_character()
    }

    fn refresh_active_external_mail(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
    ) -> Result<bool, String> {
        self.transport.refresh_active_external_mail()
    }

    fn close_session(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<(), String> {
        self.transport.close_session(owner_lease)
    }

    fn register_live_outbound(
        &self,
        _runtime: &ZoneRuntimeHandle,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        self.transport.register_live_outbound(sender)
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
        let economy_context = if matches!(
            mode,
            ZoneOwnerCommandMode::ProductionPlayer {
                authenticated: true
            }
        ) {
            runtime
                .as_mut()
                .as_any_mut()
                .downcast_mut::<SharedInProcessZoneSessionRuntime>()
                .and_then(|runtime| {
                    runtime.next_in_process_economy_execution_context(request.owner_lease())
                })
        } else {
            None
        };
        let command = request.into_command();
        if let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        {
            runtime.set_economy_execution_context(economy_context);
        }
        let result = match mode {
            ZoneOwnerCommandMode::Direct => runtime.execute_with_outcome(command),
            ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                runtime.execute_production_player_command(authenticated, command)
            }
        };
        if let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        {
            runtime.set_economy_execution_context(None);
        }
        result
    }
    fn prepare_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(owner_lease)?;
        }
        prepare_zone_teardown_checkpoint(runtime, owner_lease)
    }

    fn persist_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(prepared.owner_lease())?;
        }
        persist_zone_teardown_checkpoint(runtime, prepared)
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

    pub fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_ref()
            .map(|runtime| runtime.on_connect())
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

    pub fn active_character_checkpoint(&self) -> Result<Option<CharacterSaveRecord>, String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_ref()
            .map(|runtime| runtime.active_character_checkpoint())
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())
    }

    pub fn restore_active_character_checkpoint(
        &self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?
            .restore_active_character_checkpoint(checkpoint)
    }

    pub fn prepare_teardown_checkpoint(
        &self,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(owner_lease)?;
        }
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        prepare_zone_teardown_checkpoint(runtime, owner_lease)
    }

    pub fn persist_teardown_checkpoint(
        &self,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(prepared.owner_lease())?;
        }
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        persist_zone_teardown_checkpoint(runtime, prepared)
    }

    pub(crate) fn refresh_replica_zone_binding(&self) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        else {
            return Ok(());
        };
        runtime.refresh_replica_zone_binding()
    }

    pub(crate) fn rebind_account_store(&self, authoritative: &GatewayConfig) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        if let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        {
            runtime.rebind_account_store(authoritative);
        } else if let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<InProcessWorldRuntime>()
        {
            runtime.rebind_account_store(authoritative);
        }
        Ok(())
    }

    pub fn execute_request(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        if let Some(authority) = &self.owner_lease_authority {
            authority.validate_owner_lease(request.owner_lease())?;
        }
        self.execute_request_with_economy_context(request, true)
    }

    /// Apply a mutation that was already authenticated, fenced, ordered, and
    /// digest-verified by the replication stream.
    pub(crate) fn execute_replay_request(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.execute_request_with_economy_context(request, false)
    }

    fn execute_request_with_economy_context(
        &self,
        request: ZoneOwnerCommandRequest,
        external_commit_authorized: bool,
    ) -> Result<WorldCommandExecution, String> {
        let mode = request.mode();
        // A journal sequence proves ordering, not caller authority. Only an
        // authenticated production-player command on the active owner may
        // perform external economy effects. Direct/admin-style requests and
        // standby replay stay unfenced from the durable store even when the
        // replication layer attaches a real source sequence.
        let economy_context = match (external_commit_authorized, mode) {
            (
                true,
                ZoneOwnerCommandMode::ProductionPlayer {
                    authenticated: true,
                },
            ) => request.source_sequence().map(|source_sequence| {
                SharedAccountInventoryExecutionContext {
                    zone_id: request.owner_lease().zone_id().clone(),
                    fencing_generation: request.owner_lease().fencing_token(),
                    source_sequence,
                    created_at_ms: shared_gateway_now_ms(),
                    external_commit_authorized: true,
                }
            }),
            _ => None,
        };
        let command = request.into_command();
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let Some(runtime) = runtime.as_mut() else {
            return Err("zone owner hosted runtime was already handed off".to_string());
        };
        if let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        {
            runtime.set_economy_execution_context(economy_context);
        }
        let result = match mode {
            ZoneOwnerCommandMode::Direct => runtime.execute_with_outcome(command),
            ZoneOwnerCommandMode::ProductionPlayer { authenticated } => {
                runtime.execute_production_player_command(authenticated, command)
            }
        };
        if let Some(runtime) = runtime
            .as_mut()
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
        {
            runtime.set_economy_execution_context(None);
        }
        result
    }

    pub(crate) fn register_live_outbound(
        &self,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<SharedZoneLiveOutboundRegistration>, String> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_ref()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        let Some(ingress) = shared_zone_movement_ingress(runtime) else {
            return Ok(None);
        };
        ingress.register_live_outbound(sender)
    }
}

impl ZoneOwnerCommandClient for HostedZoneOwnerCommandClient {
    fn on_connect(&self, _runtime: &ZoneRuntimeHandle) -> Result<Vec<ServerPacket>, String> {
        HostedZoneOwnerCommandClient::on_connect(self)
    }

    fn execute(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.execute_request(request)
    }

    fn supports_typed_game_shop_purchase_outcome(&self, _runtime: &ZoneRuntimeHandle) -> bool {
        self.runtime
            .lock()
            .ok()
            .and_then(|runtime| {
                runtime
                    .as_ref()
                    .map(|runtime| runtime.supports_typed_game_shop_purchase_outcome())
            })
            .unwrap_or(false)
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

    fn active_character_checkpoint(
        &self,
        _runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<CharacterSaveRecord>, String> {
        HostedZoneOwnerCommandClient::active_character_checkpoint(self)
    }

    fn restore_active_character_checkpoint(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        HostedZoneOwnerCommandClient::restore_active_character_checkpoint(self, checkpoint)
    }

    fn prepare_teardown_checkpoint(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        HostedZoneOwnerCommandClient::prepare_teardown_checkpoint(self, owner_lease)
    }

    fn persist_teardown_checkpoint(
        &self,
        _runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        HostedZoneOwnerCommandClient::persist_teardown_checkpoint(self, prepared)
    }

    fn release_teardown_fence(&self, _runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        release_zone_teardown_fence(runtime)
    }

    fn save_active_character(&self, _runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        runtime.save_active_character()
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
    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        HostedZoneOwnerCommandClient::on_connect(self)
    }

    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.execute_request(request)
    }

    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        self.runtime
            .lock()
            .ok()
            .and_then(|runtime| {
                runtime
                    .as_ref()
                    .map(|runtime| runtime.supports_typed_game_shop_purchase_outcome())
            })
            .unwrap_or(false)
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        HostedZoneOwnerCommandClient::world_snapshot(self)
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        HostedZoneOwnerCommandClient::active_identity(self)
    }

    fn active_character_checkpoint(&self) -> Result<Option<CharacterSaveRecord>, String> {
        HostedZoneOwnerCommandClient::active_character_checkpoint(self)
    }

    fn restore_active_character_checkpoint(
        &self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        HostedZoneOwnerCommandClient::restore_active_character_checkpoint(self, checkpoint)
    }

    fn prepare_teardown_checkpoint(
        &self,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        HostedZoneOwnerCommandClient::prepare_teardown_checkpoint(self, owner_lease)
    }

    fn persist_teardown_checkpoint(&self, prepared: &PreparedZoneTeardown) -> Result<(), String> {
        HostedZoneOwnerCommandClient::persist_teardown_checkpoint(self, prepared)
    }

    fn release_teardown_fence(&self) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        release_zone_teardown_fence(runtime)
    }

    fn save_active_character(&self) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        runtime.save_active_character()
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedAccountInventoryExecutionContext {
    pub zone_id: ZoneId,
    pub fencing_generation: u64,
    pub source_sequence: u64,
    pub created_at_ms: u64,
    /// Only the finalized active owner may create an external economy
    /// transaction. Standby replay still applies the deterministic in-memory
    /// projection but must never write PostgreSQL again.
    pub external_commit_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedTradeSettlementOutcome {
    Committed,
    Duplicate,
    /// No ordered/fenced execution context was available, so no external
    /// settlement attempt was made. This is distinct from both a business
    /// rejection and an acknowledgement-unknown durable attempt.
    Deferred,
    DurableCommitted {
        event_id: String,
    },
    DurableDuplicate {
        event_id: String,
    },
    /// PostgreSQL may have committed, but the producer could not confirm the
    /// outcome. Callers must retain both debited offers and retry the same
    /// idempotent settlement; they must never roll back or reopen trading.
    OutcomeUnknown {
        idempotency_key: String,
        execution_context: SharedAccountInventoryExecutionContext,
    },
    Rejected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SharedAccountInventoryCommitOutcome {
    Confirmed(SharedAccountInventoryTransactionReceipt),
    /// No ordered/fenced execution context was available and the service did
    /// not contact the durable store or mutate the private projection.
    Deferred {
        receipt: SharedAccountInventoryTransactionReceipt,
    },
    /// PostgreSQL may have committed the immutable economy envelope, but the
    /// producer could not confirm it. The owning Zone must retain the claim and
    /// retry the same idempotency key; restoring the drop could credit it twice.
    OutcomeUnknown {
        idempotency_key: String,
        execution_context: SharedAccountInventoryExecutionContext,
        receipt: SharedAccountInventoryTransactionReceipt,
    },
}

impl SharedAccountInventoryCommitOutcome {
    fn into_receipt(self) -> SharedAccountInventoryTransactionReceipt {
        match self {
            Self::Confirmed(receipt)
            | Self::Deferred { receipt }
            | Self::OutcomeUnknown { receipt, .. } => receipt,
        }
    }
}

impl std::ops::Deref for SharedAccountInventoryCommitOutcome {
    type Target = SharedAccountInventoryTransactionReceipt;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Confirmed(receipt)
            | Self::Deferred { receipt }
            | Self::OutcomeUnknown { receipt, .. } => receipt,
        }
    }
}

impl std::ops::DerefMut for SharedAccountInventoryCommitOutcome {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Confirmed(receipt)
            | Self::Deferred { receipt }
            | Self::OutcomeUnknown { receipt, .. } => receipt,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SharedAccountInventoryCommand {
    GoldDrop {
        amount: u32,
        request_id: u64,
    },
    InventoryItemDrop {
        drop: SharedInventoryItemDrop,
        request_id: u64,
    },
    GroundDropPickup(GroundDropSnapshot),
    GroundDropClaimPickup {
        drop: GroundDropSnapshot,
        claim_idempotency_key: String,
    },
    MonsterKillAward(ZoneMonsterKillAward),
    SkillItemConsume {
        spell: Spell,
        request_id: u64,
        components: Vec<SharedSkillItemConsumptionComponent>,
    },
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
            SharedAccountInventoryCommand::GoldDrop { amount, request_id } => {
                format!("gold-drop:{request_id}:{amount}")
            }
            SharedAccountInventoryCommand::InventoryItemDrop { drop, request_id } => {
                format!(
                    "inventory-item-drop:{request_id}:{}:{}:{}",
                    drop.unique_id, drop.item_key, drop.quantity
                )
            }
            SharedAccountInventoryCommand::GroundDropPickup(drop) => {
                let payload = serde_json::to_vec(drop).ok()?;
                let payload_digest = Sha256::digest(payload)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                format!("ground-drop-pickup:{}:{payload_digest}", drop.object_id)
            }
            SharedAccountInventoryCommand::GroundDropClaimPickup {
                drop,
                claim_idempotency_key,
            } => {
                if claim_idempotency_key.trim().is_empty() {
                    return None;
                }
                let payload = serde_json::to_vec(drop).ok()?;
                let payload_digest = Sha256::digest(payload)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                format!(
                    "ground-drop-claim:{claim_idempotency_key}:{}:{payload_digest}",
                    drop.object_id
                )
            }
            SharedAccountInventoryCommand::MonsterKillAward(award) => format!(
                "monster-kill-award:{}:{}:{}:{}",
                award.monster_object_id, award.killed_at_ms, award.monster_name, award.experience
            ),
            SharedAccountInventoryCommand::SkillItemConsume {
                spell, request_id, ..
            } => {
                format!("skill-item-consume:{}:{}", *spell as u8, request_id)
            }
        };
        Some(SharedAccountInventoryCommandKey(format!(
            "{}:{}:{}",
            self.identity.account_id, self.identity.character_index, command_key
        )))
    }

    pub fn stable_idempotency_key(&self) -> String {
        self.idempotency_key()
            .expect("all shared account/inventory commands have an idempotency key")
            .0
    }
}

pub trait SharedAccountInventoryService: fmt::Debug + Send + Sync {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> mir2_simulation::SharedAccountInventoryTransactionReceipt;

    fn commit_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        _context: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        SharedAccountInventoryCommitOutcome::Confirmed(self.commit(runtime, envelope))
    }

    /// Retry a previously uncertain durable command. Durable implementations
    /// must prove that `expected_idempotency_key` is the key produced by the
    /// checkpointed execution context before contacting their store.
    fn retry_commit_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        _expected_idempotency_key: &str,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        self.commit_fenced(runtime, context, envelope)
    }

    fn bootstrap_fenced(
        &self,
        _runtime: &InProcessWorldRuntime,
        _context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        true
    }

    fn settle_trade_fenced(
        &self,
        _context: Option<&SharedAccountInventoryExecutionContext>,
        _first: &SharedTradeOffer,
        _second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        SharedTradeSettlementOutcome::Committed
    }

    /// Retry a previously uncertain durable trade under its checkpointed
    /// context and key. See `retry_commit_fenced`.
    fn retry_trade_fenced(
        &self,
        context: Option<&SharedAccountInventoryExecutionContext>,
        _expected_idempotency_key: &str,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        self.settle_trade_fenced(context, first, second)
    }

    fn reconcile_ground_drop_projections_fenced(
        &self,
        _runtime: &mut InProcessWorldRuntime,
        _context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> Vec<ServerPacket> {
        Vec::new()
    }

    fn has_pending_ground_drop_projection_fenced(
        &self,
        _runtime: &InProcessWorldRuntime,
        _context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        false
    }

    fn reconcile_trade_projections_fenced(
        &self,
        _runtime: &mut InProcessWorldRuntime,
        _context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> Vec<ServerPacket> {
        Vec::new()
    }

    fn has_pending_trade_projection_fenced(
        &self,
        _runtime: &InProcessWorldRuntime,
        _context: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        false
    }

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
    boss_reward_audits: Mutex<Vec<ZoneBossRewardAudit>>,
}

impl InProcessAccountInventoryService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn boss_reward_audits(&self) -> Vec<ZoneBossRewardAudit> {
        self.boss_reward_audits
            .lock()
            .expect("Boss reward audit mutex should not be poisoned")
            .clone()
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
            SharedAccountInventoryCommand::GoldDrop { .. } => {
                SharedAccountInventoryTransactionKind::GoldDrop
            }
            SharedAccountInventoryCommand::InventoryItemDrop { .. } => {
                SharedAccountInventoryTransactionKind::InventoryItemDrop
            }
            SharedAccountInventoryCommand::GroundDropPickup(_)
            | SharedAccountInventoryCommand::GroundDropClaimPickup { .. } => {
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
        let boss_audit = match &envelope.command {
            SharedAccountInventoryCommand::MonsterKillAward(award) => award.boss_audit.clone(),
            _ => None,
        };
        let receipt = match envelope.command {
            SharedAccountInventoryCommand::GoldDrop { amount, .. } => {
                runtime.commit_shared_gold_drop_transaction(amount)
            }
            SharedAccountInventoryCommand::InventoryItemDrop { drop, .. } => {
                runtime.commit_shared_inventory_item_drop_transaction(&drop)
            }
            SharedAccountInventoryCommand::GroundDropPickup(drop)
            | SharedAccountInventoryCommand::GroundDropClaimPickup { drop, .. } => {
                runtime.commit_shared_ground_drop_pickup_transaction(&drop)
            }
            SharedAccountInventoryCommand::MonsterKillAward(award) => runtime
                .commit_shared_monster_kill_award_transaction(
                    award.monster_object_id,
                    &award.monster_name,
                    award.experience,
                ),
            SharedAccountInventoryCommand::SkillItemConsume {
                spell, components, ..
            } => {
                if runtime.shared_skill_item_consumption_components(spell)
                    != Some(components.clone())
                {
                    return SharedAccountInventoryTransactionReceipt {
                        kind,
                        committed: false,
                        packets: Vec::new(),
                    };
                }
                runtime.commit_shared_skill_item_consumption_transaction(spell)
            }
        };
        if receipt.committed {
            if let Some(audit) = boss_audit {
                self.boss_reward_audits
                    .lock()
                    .expect("Boss reward audit mutex should not be poisoned")
                    .push(audit);
            }
        }
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
    pub affinity_key: Option<String>,
    pub explicit_line: Option<u16>,
}

impl SessionRouteRequest {
    pub fn anonymous() -> Self {
        Self::default()
    }
}

pub trait SessionRouter: Send + Sync {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId;

    fn try_route_session(
        &self,
        request: &SessionRouteRequest,
        default_zone_id: &ZoneId,
    ) -> Result<ZoneId, String> {
        Ok(self.route_session(request, default_zone_id))
    }

    /// Release any scheduler-owned placement state after a routed Session
    /// leaves its current Zone. Static routers do not retain placement state.
    fn release_session(&self, _request: &SessionRouteRequest, _now_ms: u64) -> Result<(), String> {
        Ok(())
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[doc(hidden)]
#[derive(Debug)]
pub struct SharedZoneLiveOutbound {
    registration_id: u64,
    packet: ServerPacket,
}

impl SharedZoneLiveOutbound {
    pub fn new(registration_id: u64, packet: ServerPacket) -> Self {
        Self {
            registration_id,
            packet,
        }
    }

    pub fn registration_id(&self) -> u64 {
        self.registration_id
    }

    pub fn into_packet(self) -> ServerPacket {
        self.packet
    }
}

#[doc(hidden)]
pub type SharedZoneLiveOutboundSender = TokioMpscSender<SharedZoneLiveOutbound>;

#[derive(Debug)]
struct SharedZoneLiveOutboundRecord {
    registration_id: u64,
    sender: SharedZoneLiveOutboundSender,
}

pub(crate) struct SharedZoneLiveOutboundRegistration {
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    key: ZonePresenceKey,
    registration_id: u64,
}

#[doc(hidden)]
pub trait ZoneLiveOutboundRegistration: fmt::Debug + Send {
    fn registration_id(&self) -> u64;

    fn activate(&self) {}
}

impl ZoneLiveOutboundRegistration for SharedZoneLiveOutboundRegistration {
    fn registration_id(&self) -> u64 {
        self.registration_id()
    }
}

impl SharedZoneLiveOutboundRegistration {
    pub(crate) fn registration_id(&self) -> u64 {
        self.registration_id
    }
}

impl fmt::Debug for SharedZoneLiveOutboundRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedZoneLiveOutboundRegistration")
            .field("key", &self.key)
            .field("registration_id", &self.registration_id)
            .finish_non_exhaustive()
    }
}

impl Drop for SharedZoneLiveOutboundRegistration {
    fn drop(&mut self) {
        if let Ok(mut zone_state) = self.zone_state.lock() {
            zone_state.unregister_live_zone_outbound(&self.key, self.registration_id);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZonePlayerPresence {
    zone_object_id: u32,
    map_file_name: String,
    entity: WorldEntitySnapshot,
    free_bag_slots: u16,
    #[serde(default)]
    pk_points: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SharedDeathDropAnchor {
    monster_name: Option<String>,
    location: Point,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SharedDeadEntityState {
    location: Option<Point>,
    direction: Option<MirDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedItemRentalInvite {
    partner_name: String,
    renting: bool,
}

/// A two-party trade whose PostgreSQL COMMIT acknowledgement was lost. Both
/// offers have already been debited from their private runtimes, so this record
/// is the recovery authority until the same idempotent settlement is confirmed
/// durable or confirmed absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UnresolvedSharedTradeSettlement {
    idempotency_key: String,
    /// Exact context that produced the uncertain durable envelope. Older
    /// checkpoints omit it and therefore remain fail-closed instead of being
    /// retried under a newly generated key.
    #[serde(default)]
    execution_context: Option<SharedAccountInventoryExecutionContext>,
    first_key: ZonePresenceKey,
    second_key: ZonePresenceKey,
    first_offer: SharedTradeOffer,
    second_offer: SharedTradeOffer,
}

impl UnresolvedSharedTradeSettlement {
    fn recovery_key(&self) -> String {
        format!(
            "{}|{}:{}:{}|{}:{}:{}",
            self.idempotency_key,
            self.first_key.account_id,
            self.first_key.character_index,
            self.first_offer.settlement_nonce,
            self.second_key.account_id,
            self.second_key.character_index,
            self.second_offer.settlement_nonce
        )
    }

    fn involves(&self, key: &ZonePresenceKey) -> bool {
        &self.first_key == key || &self.second_key == key
    }
}

/// A ground-drop settlement whose durable outcome is unknown, detached from
/// the historical Gateway session while the Zone retains the removed-object
/// tombstone. Recovery is owned by account/character and the full Zone key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UnresolvedGroundDropSettlement {
    #[serde(default)]
    idempotency_key: Option<String>,
    /// Exact context that produced `idempotency_key`; see the trade record.
    #[serde(default)]
    execution_context: Option<SharedAccountInventoryExecutionContext>,
    presence_key: ZonePresenceKey,
    zone_key: ZoneKey,
    ticket: GroundDropClaimTicket,
}

impl UnresolvedGroundDropSettlement {
    fn recovery_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}:{}|{}",
            self.zone_key.shard_id,
            self.zone_key.map_file_name.to_ascii_lowercase(),
            self.zone_key.channel_id,
            self.zone_key.instance_id,
            self.presence_key.account_id,
            self.presence_key.character_index,
            self.ticket.idempotency_key
        )
    }

    fn involves(&self, key: &ZonePresenceKey) -> bool {
        &self.presence_key == key
    }
}

const SHARED_ZONE_STATE_CHECKPOINT_VERSION: u32 = 3;
const SHARED_ZONE_FACTORY_CHECKPOINT_VERSION: u32 = 2;
const MAX_PENDING_ZONE_PACKETS_PER_PLAYER: usize =
    crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES;

#[derive(Debug)]
struct SharedInProcessZoneState {
    next_zone_object_id: u32,
    next_live_outbound_registration_id: u64,
    #[cfg(test)]
    zone_cadence_tick_count: u64,
    /// Monotonic command sequence used by the local authoritative Zone owner
    /// for fenced economy side effects. It is checkpointed with Zone state so
    /// every in-process session sharing this Zone consumes one ordered stream.
    next_economy_source_sequence: u64,
    zone_manager: ZoneManager,
    zone_sessions: BTreeMap<ZonePresenceKey, SessionId>,
    zone_session_keys: BTreeMap<SessionId, ZonePresenceKey>,
    pending_zone_packets: BTreeMap<ZonePresenceKey, Vec<ServerPacket>>,
    pending_zone_transforms: BTreeMap<ZonePresenceKey, (Point, MirDirection)>,
    pending_zone_shout_consumes: BTreeMap<ZonePresenceKey, (bool, bool)>,
    pending_zone_ground_drop_claims: BTreeMap<ZonePresenceKey, Vec<GroundDropClaimTicket>>,
    pending_zone_monster_kill_awards: BTreeMap<ZonePresenceKey, Vec<ZoneMonsterKillAward>>,
    pending_zone_player_damages: BTreeMap<ZonePresenceKey, Vec<i32>>,
    pending_zone_player_heals: BTreeMap<ZonePresenceKey, Vec<i32>>,
    teardown_fences: BTreeSet<ZonePresenceKey>,
    live_zone_outbounds: BTreeMap<ZonePresenceKey, SharedZoneLiveOutboundRecord>,
    players: BTreeMap<ZonePresenceKey, ZonePlayerPresence>,
    maps: BTreeMap<String, ZoneMapSnapshotLayer>,
    trade_offers: BTreeMap<ZonePresenceKey, SharedTradeOffer>,
    pending_trade_deliveries: BTreeMap<ZonePresenceKey, Vec<SharedTradeOffer>>,
    pending_trade_rollbacks: BTreeMap<ZonePresenceKey, Vec<SharedTradeOffer>>,
    unresolved_ground_drop_settlements: BTreeMap<String, UnresolvedGroundDropSettlement>,
    unresolved_trade_settlements: BTreeMap<String, UnresolvedSharedTradeSettlement>,
    pending_rental_invites: BTreeMap<ZonePresenceKey, Vec<SharedItemRentalInvite>>,
    pending_rental_cancels: BTreeMap<ZonePresenceKey, usize>,
    rental_item_offers: BTreeMap<ZonePresenceKey, SharedItemRentalItemOffer>,
    rental_fee_offers: BTreeMap<ZonePresenceKey, SharedItemRentalFeeOffer>,
    pending_rental_deliveries: BTreeMap<ZonePresenceKey, Vec<SharedItemRentalDelivery>>,
    npc_saved_values: BTreeMap<SharedNpcSavedValueKey, SharedNpcSavedValue>,
    npc_random_seed: Option<u64>,
}

/// Serde adapter for binary checkpoint blobs. `Vec<u8>` payloads (zone manager
/// images, packet frames) default to JSON number arrays that inflate a
/// multi-megabyte world image roughly 4-5x; base64 keeps them compact.
pub(crate) mod base64_bytes {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        STANDARD
            .decode(encoded.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedInProcessZoneStateCheckpoint {
    version: u32,
    next_zone_object_id: u32,
    #[serde(default)]
    next_economy_source_sequence: u64,
    next_live_outbound_registration_id: u64,
    #[serde(with = "crate::routing::base64_bytes", default)]
    zone_manager_bytes: Vec<u8>,
    zone_sessions: Vec<(ZonePresenceKey, SessionId)>,
    zone_session_keys: Vec<(SessionId, ZonePresenceKey)>,
    pending_zone_packet_frames: Vec<(ZonePresenceKey, Vec<Vec<u8>>)>,
    pending_zone_transforms: Vec<(ZonePresenceKey, (Point, MirDirection))>,
    pending_zone_shout_consumes: Vec<(ZonePresenceKey, (bool, bool))>,
    pending_zone_ground_drop_claims: Vec<(ZonePresenceKey, Vec<GroundDropClaimTicket>)>,
    pending_zone_monster_kill_awards: Vec<(ZonePresenceKey, Vec<ZoneMonsterKillAward>)>,
    pending_zone_player_damages: Vec<(ZonePresenceKey, Vec<i32>)>,
    pending_zone_player_heals: Vec<(ZonePresenceKey, Vec<i32>)>,
    #[serde(default)]
    teardown_fences: Vec<ZonePresenceKey>,
    players: Vec<(ZonePresenceKey, ZonePlayerPresence)>,
    maps: BTreeMap<String, ZoneMapSnapshotLayer>,
    trade_offers: Vec<(ZonePresenceKey, SharedTradeOffer)>,
    pending_trade_deliveries: Vec<(ZonePresenceKey, Vec<SharedTradeOffer>)>,
    pending_trade_rollbacks: Vec<(ZonePresenceKey, Vec<SharedTradeOffer>)>,
    #[serde(default)]
    unresolved_ground_drop_settlements: Vec<UnresolvedGroundDropSettlement>,
    #[serde(default)]
    unresolved_trade_settlements: Vec<UnresolvedSharedTradeSettlement>,
    pending_rental_invites: Vec<(ZonePresenceKey, Vec<SharedItemRentalInvite>)>,
    pending_rental_cancels: Vec<(ZonePresenceKey, usize)>,
    rental_item_offers: Vec<(ZonePresenceKey, SharedItemRentalItemOffer)>,
    rental_fee_offers: Vec<(ZonePresenceKey, SharedItemRentalFeeOffer)>,
    pending_rental_deliveries: Vec<(ZonePresenceKey, Vec<SharedItemRentalDelivery>)>,
    npc_saved_values: Vec<(SharedNpcSavedValueKey, SharedNpcSavedValue)>,
    npc_random_seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedInProcessZoneFactoryCheckpoint {
    version: u32,
    zones: BTreeMap<ZoneId, SharedInProcessZoneStateCheckpoint>,
}

impl SharedInProcessZoneStateCheckpoint {
    /// Convert a full runtime image into a process-independent world image.
    ///
    /// Session ownership belongs to the Gateway process. Restoring it from the
    /// World Director checkpoint creates players that have no corresponding
    /// `ZoneHostSession`, so their outbound packets can never be drained.
    fn into_world_only(self) -> Result<Self, String> {
        self.into_world_only_with_root_policy(false)
    }

    fn into_verified_world_only(self) -> Result<Self, String> {
        self.into_world_only_with_root_policy(true)
    }

    fn into_world_only_with_root_policy(
        mut self,
        verified_outer_checkpoint: bool,
    ) -> Result<Self, String> {
        let mut zone_manager = if verified_outer_checkpoint {
            ZoneManager::restore_verified_world_checkpoint(&self.zone_manager_bytes)?
        } else {
            ZoneManager::restore_checkpoint(&self.zone_manager_bytes)?
        };
        let mut unresolved_ground = self
            .unresolved_ground_drop_settlements
            .drain(..)
            .map(|settlement| (settlement.recovery_key(), settlement))
            .collect::<BTreeMap<_, _>>();
        for (zone_key, session_id, ticket) in zone_manager.detach_all_ground_drop_claims() {
            let presence_key = self
                .zone_session_keys
                .iter()
                .find_map(|(candidate_session, key)| {
                    (candidate_session == &session_id).then_some(key.clone())
                })
                .ok_or_else(|| {
                    format!(
                        "world-only checkpoint cannot detach ground claim without presence for {}",
                        session_id.as_str()
                    )
                })?;
            let settlement = UnresolvedGroundDropSettlement {
                idempotency_key: None,
                execution_context: None,
                presence_key,
                zone_key,
                ticket,
            };
            let recovery_key = settlement.recovery_key();
            if let Some(existing) = unresolved_ground.insert(recovery_key, settlement.clone()) {
                if existing != settlement {
                    return Err(
                        "world-only checkpoint has conflicting detached ground settlements"
                            .to_string(),
                    );
                }
            }
        }
        self.unresolved_ground_drop_settlements = unresolved_ground.into_values().collect();
        zone_manager.leave_all_sessions();
        self.zone_manager_bytes = zone_manager.checkpoint_bytes()?;

        let player_object_ids = self
            .players
            .iter()
            .map(|(_, presence)| presence.zone_object_id)
            .collect::<BTreeSet<_>>();
        let player_names = self
            .players
            .iter()
            .map(|(_, presence)| presence.entity.name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        for map in self.maps.values_mut() {
            let transient_entity_ids = map
                .entities
                .values()
                .filter(|entity| {
                    player_object_ids.contains(&entity.object_id)
                        || matches!(
                            entity.kind,
                            WorldEntityKind::SelfPlayer | WorldEntityKind::Player
                        )
                        || entity.owner_name.as_ref().is_some_and(|owner_name| {
                            player_names.contains(&owner_name.to_ascii_lowercase())
                        })
                })
                .map(|entity| entity.object_id)
                .chain(player_object_ids.iter().copied())
                .collect::<BTreeSet<_>>();
            map.entities
                .retain(|object_id, _| !transient_entity_ids.contains(object_id));
            map.removed_entity_ids
                .retain(|object_id| !transient_entity_ids.contains(object_id));
            map.dead_entity_ids
                .retain(|object_id, _| !transient_entity_ids.contains(object_id));
            map.committed_death_drop_anchors
                .retain(|object_id, _| !transient_entity_ids.contains(object_id));
            map.harvested_entity_ids
                .retain(|object_id| !transient_entity_ids.contains(object_id));
            map.revived_entity_ids
                .retain(|object_id| !transient_entity_ids.contains(object_id));
        }

        self.next_live_outbound_registration_id = 0;
        self.zone_sessions.clear();
        self.zone_session_keys.clear();
        self.pending_zone_packet_frames.clear();
        self.pending_zone_transforms.clear();
        self.pending_zone_shout_consumes.clear();
        self.pending_zone_ground_drop_claims.clear();
        self.pending_zone_monster_kill_awards.clear();
        self.pending_zone_player_damages.clear();
        self.pending_zone_player_heals.clear();
        self.teardown_fences.clear();
        self.players.clear();
        self.trade_offers.clear();
        self.pending_trade_deliveries.clear();
        self.pending_trade_rollbacks.clear();
        self.pending_rental_invites.clear();
        self.pending_rental_cancels.clear();
        self.rental_item_offers.clear();
        self.rental_fee_offers.clear();
        self.pending_rental_deliveries.clear();
        Ok(self)
    }
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
        item_param: u8,
    },
}

#[derive(Debug, Clone)]
struct ZoneNativePlayerAttack {
    object_id: u32,
    is_player_target: bool,
    is_red_player_target: bool,
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
            next_live_outbound_registration_id: 0,
            #[cfg(test)]
            zone_cadence_tick_count: 0,
            next_economy_source_sequence: 0,
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
            teardown_fences: BTreeSet::new(),
            live_zone_outbounds: BTreeMap::new(),
            players: BTreeMap::new(),
            maps: BTreeMap::new(),
            trade_offers: BTreeMap::new(),
            pending_trade_deliveries: BTreeMap::new(),
            pending_trade_rollbacks: BTreeMap::new(),
            unresolved_ground_drop_settlements: BTreeMap::new(),
            unresolved_trade_settlements: BTreeMap::new(),
            pending_rental_invites: BTreeMap::new(),
            pending_rental_cancels: BTreeMap::new(),
            rental_item_offers: BTreeMap::new(),
            rental_fee_offers: BTreeMap::new(),
            pending_rental_deliveries: BTreeMap::new(),
            npc_saved_values: BTreeMap::new(),
            npc_random_seed: None,
        }
    }

    fn next_economy_source_sequence(&mut self) -> Option<u64> {
        let next = self.next_economy_source_sequence.checked_add(1)?;
        self.next_economy_source_sequence = next;
        Some(next)
    }

    fn checkpoint(&self) -> Result<SharedInProcessZoneStateCheckpoint, String> {
        Ok(SharedInProcessZoneStateCheckpoint {
            version: SHARED_ZONE_STATE_CHECKPOINT_VERSION,
            next_zone_object_id: self.next_zone_object_id,
            next_economy_source_sequence: self.next_economy_source_sequence,
            next_live_outbound_registration_id: self.next_live_outbound_registration_id,
            zone_manager_bytes: self.zone_manager.checkpoint_bytes()?,
            zone_sessions: self.zone_sessions.clone().into_iter().collect(),
            zone_session_keys: self.zone_session_keys.clone().into_iter().collect(),
            pending_zone_packet_frames: self
                .pending_zone_packets
                .iter()
                .map(|(key, packets)| {
                    let frames = packets
                        .iter()
                        .map(|packet| {
                            encode_server_packet(packet).map_err(|error| {
                                format!("failed to encode pending Zone packet: {error}")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((key.clone(), frames))
                })
                .collect::<Result<Vec<_>, String>>()?,
            pending_zone_transforms: self.pending_zone_transforms.clone().into_iter().collect(),
            pending_zone_shout_consumes: self
                .pending_zone_shout_consumes
                .clone()
                .into_iter()
                .collect(),
            pending_zone_ground_drop_claims: self
                .pending_zone_ground_drop_claims
                .clone()
                .into_iter()
                .collect(),
            pending_zone_monster_kill_awards: self
                .pending_zone_monster_kill_awards
                .clone()
                .into_iter()
                .collect(),
            pending_zone_player_damages: self
                .pending_zone_player_damages
                .clone()
                .into_iter()
                .collect(),
            pending_zone_player_heals: self.pending_zone_player_heals.clone().into_iter().collect(),
            teardown_fences: self.teardown_fences.clone().into_iter().collect(),
            players: self.players.clone().into_iter().collect(),
            maps: self.maps.clone(),
            trade_offers: self.trade_offers.clone().into_iter().collect(),
            pending_trade_deliveries: self.pending_trade_deliveries.clone().into_iter().collect(),
            pending_trade_rollbacks: self.pending_trade_rollbacks.clone().into_iter().collect(),
            unresolved_ground_drop_settlements: self
                .unresolved_ground_drop_settlements
                .values()
                .cloned()
                .collect(),
            unresolved_trade_settlements: self
                .unresolved_trade_settlements
                .values()
                .cloned()
                .collect(),
            pending_rental_invites: self.pending_rental_invites.clone().into_iter().collect(),
            pending_rental_cancels: self.pending_rental_cancels.clone().into_iter().collect(),
            rental_item_offers: self.rental_item_offers.clone().into_iter().collect(),
            rental_fee_offers: self.rental_fee_offers.clone().into_iter().collect(),
            pending_rental_deliveries: self.pending_rental_deliveries.clone().into_iter().collect(),
            npc_saved_values: self.npc_saved_values.clone().into_iter().collect(),
            npc_random_seed: self.npc_random_seed,
        })
    }

    fn world_checkpoint(&self) -> Result<SharedInProcessZoneStateCheckpoint, String> {
        SharedInProcessZoneStateCheckpoint {
            version: SHARED_ZONE_STATE_CHECKPOINT_VERSION,
            next_zone_object_id: self.next_zone_object_id,
            next_economy_source_sequence: self.next_economy_source_sequence,
            next_live_outbound_registration_id: 0,
            zone_manager_bytes: self.zone_manager.checkpoint_bytes()?,
            zone_sessions: self.zone_sessions.clone().into_iter().collect(),
            zone_session_keys: self.zone_session_keys.clone().into_iter().collect(),
            pending_zone_packet_frames: Vec::new(),
            pending_zone_transforms: Vec::new(),
            pending_zone_shout_consumes: Vec::new(),
            pending_zone_ground_drop_claims: Vec::new(),
            pending_zone_monster_kill_awards: Vec::new(),
            pending_zone_player_damages: Vec::new(),
            pending_zone_player_heals: Vec::new(),
            teardown_fences: Vec::new(),
            players: self.players.clone().into_iter().collect(),
            maps: self.maps.clone(),
            trade_offers: Vec::new(),
            pending_trade_deliveries: Vec::new(),
            pending_trade_rollbacks: Vec::new(),
            unresolved_ground_drop_settlements: self
                .unresolved_ground_drop_settlements
                .values()
                .cloned()
                .collect(),
            unresolved_trade_settlements: self
                .unresolved_trade_settlements
                .values()
                .cloned()
                .collect(),
            pending_rental_invites: Vec::new(),
            pending_rental_cancels: Vec::new(),
            rental_item_offers: Vec::new(),
            rental_fee_offers: Vec::new(),
            pending_rental_deliveries: Vec::new(),
            npc_saved_values: self.npc_saved_values.clone().into_iter().collect(),
            npc_random_seed: self.npc_random_seed,
        }
        .into_world_only()
    }

    fn restore(checkpoint: SharedInProcessZoneStateCheckpoint) -> Result<Self, String> {
        if checkpoint.version != SHARED_ZONE_STATE_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported shared Zone state checkpoint version {}, expected {}",
                checkpoint.version, SHARED_ZONE_STATE_CHECKPOINT_VERSION
            ));
        }
        let zone_sessions = checkpoint
            .zone_sessions
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        let zone_session_keys = checkpoint
            .zone_session_keys
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        if zone_sessions.len() != checkpoint.zone_sessions.len()
            || zone_session_keys.len() != checkpoint.zone_session_keys.len()
        {
            return Err("shared Zone checkpoint contains duplicate session mappings".to_string());
        }
        for (key, session_id) in &zone_sessions {
            if zone_session_keys.get(session_id) != Some(key) {
                return Err(format!(
                    "shared Zone checkpoint has inconsistent session mapping for {}",
                    session_id.as_str()
                ));
            }
        }
        for (session_id, key) in &zone_session_keys {
            if zone_sessions.get(key) != Some(session_id) {
                return Err(format!(
                    "shared Zone checkpoint has inconsistent presence mapping for {}",
                    session_id.as_str()
                ));
            }
        }
        let zone_manager = ZoneManager::restore_checkpoint(&checkpoint.zone_manager_bytes)?;
        let players = checkpoint
            .players
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        if players.len() != checkpoint.players.len() {
            return Err("shared Zone checkpoint contains duplicate player presences".to_string());
        }
        let mut pending_zone_ground_drop_claims = checkpoint
            .pending_zone_ground_drop_claims
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        if pending_zone_ground_drop_claims.len() != checkpoint.pending_zone_ground_drop_claims.len()
        {
            return Err(
                "shared Zone checkpoint contains duplicate pending drop-claim presences"
                    .to_string(),
            );
        }
        for (key, tickets) in &pending_zone_ground_drop_claims {
            let session_id = zone_sessions.get(key).ok_or_else(|| {
                format!(
                    "shared Zone checkpoint has pending drop claim without presence for {}/{}",
                    key.account_id, key.character_index
                )
            })?;
            if !players.contains_key(key) {
                return Err(format!(
                    "shared Zone checkpoint has pending drop claim without player for {}/{}",
                    key.account_id, key.character_index
                ));
            }
            for ticket in tickets {
                if &ticket.session_id != session_id {
                    return Err(format!(
                        "shared Zone checkpoint pending drop claim session mismatch for {}/{}",
                        key.account_id, key.character_index
                    ));
                }
                if !zone_manager.has_pending_ground_drop_claim_ticket(session_id, ticket) {
                    return Err(format!(
                        "shared Zone checkpoint pending drop claim is absent or mismatched in Zone for {}/{} object {}",
                        key.account_id, key.character_index, ticket.object_id
                    ));
                }
            }
        }
        // The Zone checkpoint is authoritative for a claim that was accepted
        // before the Gateway had materialized its settlement queue. Restore
        // those exact tickets, but never invent a Gateway presence: every Zone
        // ticket must still map to the same session and player.
        for (session_id, ticket) in zone_manager.pending_ground_drop_claim_tickets() {
            let key = zone_session_keys.get(&session_id).ok_or_else(|| {
                format!(
                    "shared Zone checkpoint has Zone pending drop claim without session mapping for {}",
                    session_id.as_str()
                )
            })?;
            if zone_sessions.get(key) != Some(&session_id) {
                return Err(format!(
                    "shared Zone checkpoint has Zone pending drop claim with inconsistent presence mapping for {}",
                    session_id.as_str()
                ));
            }
            if !players.contains_key(key) {
                return Err(format!(
                    "shared Zone checkpoint has Zone pending drop claim without player for {}/{}",
                    key.account_id, key.character_index
                ));
            }
            let tickets = pending_zone_ground_drop_claims
                .entry(key.clone())
                .or_default();
            if !tickets.contains(&ticket) {
                tickets.push(ticket);
            }
        }
        let unresolved_ground_count = checkpoint.unresolved_ground_drop_settlements.len();
        let unresolved_ground_drop_settlements = checkpoint
            .unresolved_ground_drop_settlements
            .into_iter()
            .map(|settlement| {
                if settlement.presence_key.account_id.trim().is_empty()
                    || settlement
                        .idempotency_key
                        .as_ref()
                        .is_some_and(|key| key.trim().is_empty())
                    || !zone_manager.has_detached_ground_drop_claim_ticket(
                        &settlement.zone_key,
                        &settlement.ticket,
                    )
                {
                    return Err(
                        "shared Zone checkpoint contains invalid detached ground settlement"
                            .to_string(),
                    );
                }
                Ok((settlement.recovery_key(), settlement))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        if unresolved_ground_drop_settlements.len() != unresolved_ground_count {
            return Err(
                "shared Zone checkpoint contains duplicate detached ground settlements".to_string(),
            );
        }
        let unresolved_count = checkpoint.unresolved_trade_settlements.len();
        let unresolved_trade_settlements = checkpoint
            .unresolved_trade_settlements
            .into_iter()
            .map(|settlement| {
                if settlement.idempotency_key.trim().is_empty()
                    || settlement.first_key == settlement.second_key
                    || settlement.first_key.account_id != settlement.first_offer.account_id
                    || settlement.first_key.character_index
                        != settlement.first_offer.character_index
                    || settlement.second_key.account_id != settlement.second_offer.account_id
                    || settlement.second_key.character_index
                        != settlement.second_offer.character_index
                    || !settlement
                        .first_offer
                        .partner_name
                        .eq_ignore_ascii_case(&settlement.second_offer.character_name)
                    || !settlement
                        .second_offer
                        .partner_name
                        .eq_ignore_ascii_case(&settlement.first_offer.character_name)
                {
                    return Err(
                        "shared Zone checkpoint contains invalid unresolved trade settlement"
                            .to_string(),
                    );
                }
                Ok((settlement.recovery_key(), settlement))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        if unresolved_trade_settlements.len() != unresolved_count {
            return Err(
                "shared Zone checkpoint contains duplicate unresolved trade settlements"
                    .to_string(),
            );
        }
        Ok(Self {
            next_zone_object_id: checkpoint.next_zone_object_id,
            next_live_outbound_registration_id: checkpoint.next_live_outbound_registration_id,
            #[cfg(test)]
            zone_cadence_tick_count: 0,
            next_economy_source_sequence: checkpoint.next_economy_source_sequence,
            zone_manager,
            zone_sessions,
            zone_session_keys,
            pending_zone_packets: checkpoint
                .pending_zone_packet_frames
                .into_iter()
                .map(|(key, frames)| {
                    let retained_from = frames
                        .len()
                        .saturating_sub(MAX_PENDING_ZONE_PACKETS_PER_PLAYER);
                    let packets = frames
                        .into_iter()
                        .skip(retained_from)
                        .map(|frame| {
                            decode_server_packet(&frame).map_err(|error| {
                                format!("failed to decode pending Zone packet: {error}")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((key, packets))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()?,
            pending_zone_transforms: checkpoint.pending_zone_transforms.into_iter().collect(),
            pending_zone_shout_consumes: checkpoint
                .pending_zone_shout_consumes
                .into_iter()
                .collect(),
            pending_zone_ground_drop_claims,
            pending_zone_monster_kill_awards: checkpoint
                .pending_zone_monster_kill_awards
                .into_iter()
                .collect(),
            pending_zone_player_damages: checkpoint
                .pending_zone_player_damages
                .into_iter()
                .collect(),
            pending_zone_player_heals: checkpoint.pending_zone_player_heals.into_iter().collect(),
            teardown_fences: checkpoint.teardown_fences.into_iter().collect(),
            live_zone_outbounds: BTreeMap::new(),
            players,
            maps: checkpoint.maps,
            trade_offers: checkpoint.trade_offers.into_iter().collect(),
            pending_trade_deliveries: checkpoint.pending_trade_deliveries.into_iter().collect(),
            pending_trade_rollbacks: checkpoint.pending_trade_rollbacks.into_iter().collect(),
            unresolved_ground_drop_settlements,
            unresolved_trade_settlements,
            pending_rental_invites: checkpoint.pending_rental_invites.into_iter().collect(),
            pending_rental_cancels: checkpoint.pending_rental_cancels.into_iter().collect(),
            rental_item_offers: checkpoint.rental_item_offers.into_iter().collect(),
            rental_fee_offers: checkpoint.rental_fee_offers.into_iter().collect(),
            pending_rental_deliveries: checkpoint.pending_rental_deliveries.into_iter().collect(),
            npc_saved_values: checkpoint.npc_saved_values.into_iter().collect(),
            npc_random_seed: checkpoint.npc_random_seed,
        })
    }

    #[cfg(test)]
    fn upsert_player(
        &mut self,
        key: ZonePresenceKey,
        character_name: &str,
        map_file_name: String,
        self_entity: WorldEntitySnapshot,
        free_bag_slots: u16,
    ) -> u32 {
        self.upsert_player_with_transform_policy(
            key,
            character_name,
            map_file_name,
            self_entity,
            free_bag_slots,
            0,
            true,
        )
    }

    fn upsert_player_with_transform_policy(
        &mut self,
        key: ZonePresenceKey,
        character_name: &str,
        map_file_name: String,
        self_entity: WorldEntitySnapshot,
        free_bag_slots: u16,
        pk_points: i32,
        preserve_existing_transform: bool,
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
            if preserve_existing_transform && existing.map_file_name == map_file_name {
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
                pk_points,
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
        self.teardown_fences.remove(key);
        self.live_zone_outbounds.remove(key);
    }

    fn begin_teardown_fence(&mut self, key: &ZonePresenceKey) -> Result<(), String> {
        if !self.players.contains_key(key) || !self.zone_sessions.contains_key(key) {
            return Err("cannot fence a missing shared Zone presence".to_string());
        }
        self.teardown_fences.insert(key.clone());
        // Once the socket has closed, no realtime packet may bypass the
        // deterministic teardown drain through a stale live registration.
        self.live_zone_outbounds.remove(key);
        Ok(())
    }

    fn release_teardown_fence(&mut self, key: &ZonePresenceKey) {
        self.teardown_fences.remove(key);
    }

    fn teardown_fenced(&self, key: &ZonePresenceKey) -> bool {
        self.teardown_fences.contains(key)
    }

    fn any_teardown_fenced(&self) -> bool {
        !self.teardown_fences.is_empty()
    }
    fn command_mutates_teardown_fence(&self, command: &ZoneCommand) -> bool {
        if self.teardown_fences.is_empty() {
            return false;
        }

        let source_session_id = match command {
            ZoneCommand::Join(join) => Some(&join.session_id),
            ZoneCommand::Leave { session_id }
            | ZoneCommand::Walk { session_id, .. }
            | ZoneCommand::Run { session_id, .. }
            | ZoneCommand::Turn { session_id, .. }
            | ZoneCommand::TeleportToNpc { session_id, .. }
            | ZoneCommand::UpdateChatProfile { session_id, .. }
            | ZoneCommand::UpdatePlayerCombatStats { session_id, .. }
            | ZoneCommand::SyncPlayerCombatState { session_id, .. }
            | ZoneCommand::SyncPlayerTransform { session_id, .. }
            | ZoneCommand::SyncPlayerVitals { session_id, .. }
            | ZoneCommand::Chat { session_id, .. }
            | ZoneCommand::BroadcastPackets { session_id, .. }
            | ZoneCommand::SyncSharedObjects { session_id, .. }
            | ZoneCommand::BroadcastSharedObjectPackets { session_id, .. }
            | ZoneCommand::SyncGroundDrops { session_id, .. }
            | ZoneCommand::SpawnMonster { session_id, .. }
            | ZoneCommand::SyncNativeMonsters { session_id, .. }
            | ZoneCommand::PlayerAttackObject { session_id, .. }
            | ZoneCommand::PlayerAttackMaterializedObject { session_id, .. }
            | ZoneCommand::PlayerRangeAttackObject { session_id, .. }
            | ZoneCommand::PlayerRangeAttackMaterializedObject { session_id, .. }
            | ZoneCommand::PlayerCastMagic { session_id, .. }
            | ZoneCommand::PlayerCastMagicWithItem { session_id, .. }
            | ZoneCommand::ResolveReincarnation { session_id, .. }
            | ZoneCommand::ClaimGroundDrop { session_id, .. }
            | ZoneCommand::ClaimNearestGroundDrop { session_id, .. }
            | ZoneCommand::CommitGroundDropClaim { session_id, .. }
            | ZoneCommand::CommitGroundDropClaimWithTicket { session_id, .. }
            | ZoneCommand::CancelGroundDropClaim { session_id, .. }
            | ZoneCommand::CancelGroundDropClaimWithTicket { session_id, .. }
            | ZoneCommand::CancelPendingMovement { session_id }
            | ZoneCommand::TickPlayerMovement { session_id, .. }
            | ZoneCommand::OpenDoor { session_id, .. }
            | ZoneCommand::ConfigureHazards { session_id, .. } => Some(session_id),
            ZoneCommand::Tick { .. } => None,
        };
        if source_session_id.is_some_and(|session_id| {
            self.zone_session_keys
                .get(session_id)
                .is_some_and(|key| self.teardown_fenced(key))
        }) {
            return true;
        }

        let target_object_id = match command {
            ZoneCommand::PlayerAttackObject { object_id, .. }
            | ZoneCommand::PlayerAttackMaterializedObject { object_id, .. }
            | ZoneCommand::PlayerRangeAttackObject { object_id, .. }
            | ZoneCommand::PlayerRangeAttackMaterializedObject { object_id, .. }
            | ZoneCommand::PlayerCastMagic { object_id, .. }
            | ZoneCommand::PlayerCastMagicWithItem { object_id, .. } => Some(*object_id),
            _ => None,
        };
        if target_object_id.is_some_and(|object_id| {
            self.teardown_fences.iter().any(|key| {
                self.players
                    .get(key)
                    .is_some_and(|presence| presence.zone_object_id == object_id)
            })
        }) {
            return true;
        }

        matches!(command, ZoneCommand::Tick { .. })
    }

    fn prepend_zone_monster_kill_awards(
        &mut self,
        key: ZonePresenceKey,
        mut awards: Vec<ZoneMonsterKillAward>,
    ) {
        if awards.is_empty() {
            return;
        }
        if let Some(mut later) = self.pending_zone_monster_kill_awards.remove(&key) {
            awards.append(&mut later);
        }
        self.pending_zone_monster_kill_awards.insert(key, awards);
    }

    fn zone_session_id_for_key(key: &ZonePresenceKey) -> SessionId {
        SessionId::new(format!("{}:{}", key.account_id, key.character_index))
    }

    fn queue_zone_packets(&mut self, key: ZonePresenceKey, packets: Vec<ServerPacket>) {
        if packets.is_empty() {
            return;
        }
        for packet in packets {
            let packet = match self.try_push_live_zone_outbound(&key, packet) {
                Ok(()) => continue,
                Err(packet) => packet,
            };
            let pending = self.pending_zone_packets.entry(key.clone()).or_default();
            if let Some(object_id) = coalesced_zone_movement_object_id(&packet) {
                pending
                    .retain(|queued| coalesced_zone_movement_object_id(queued) != Some(object_id));
            }
            pending.push(packet);
            let overflow = pending
                .len()
                .saturating_sub(MAX_PENDING_ZONE_PACKETS_PER_PLAYER);
            if overflow > 0 {
                pending.drain(..overflow);
            }
        }
    }

    fn register_live_zone_outbound(
        &mut self,
        key: ZonePresenceKey,
        sender: SharedZoneLiveOutboundSender,
    ) -> u64 {
        self.next_live_outbound_registration_id = self
            .next_live_outbound_registration_id
            .saturating_add(1)
            .max(1);
        let registration_id = self.next_live_outbound_registration_id;
        self.live_zone_outbounds.insert(
            key,
            SharedZoneLiveOutboundRecord {
                registration_id,
                sender,
            },
        );
        registration_id
    }

    fn unregister_live_zone_outbound(&mut self, key: &ZonePresenceKey, registration_id: u64) {
        if self
            .live_zone_outbounds
            .get(key)
            .is_some_and(|record| record.registration_id == registration_id)
        {
            self.live_zone_outbounds.remove(key);
        }
    }

    fn try_push_live_zone_outbound(
        &mut self,
        key: &ZonePresenceKey,
        packet: ServerPacket,
    ) -> Result<(), ServerPacket> {
        if !is_realtime_zone_live_packet(&packet) {
            return Err(packet);
        }
        let Some((registration_id, sender)) = self
            .live_zone_outbounds
            .get(key)
            .map(|record| (record.registration_id, record.sender.clone()))
        else {
            return Err(packet);
        };
        let outbound = SharedZoneLiveOutbound {
            registration_id,
            packet,
        };
        match sender.try_send(outbound) {
            Ok(()) => Ok(()),
            Err(TokioTrySendError::Full(outbound)) => Err(outbound.packet),
            Err(TokioTrySendError::Closed(outbound)) => {
                self.unregister_live_zone_outbound(key, registration_id);
                Err(outbound.packet)
            }
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

    fn queue_zone_ground_drop_claim(
        &mut self,
        key: ZonePresenceKey,
        ticket: GroundDropClaimTicket,
    ) {
        self.pending_zone_ground_drop_claims
            .entry(key)
            .or_default()
            .push(ticket);
    }

    fn detach_unresolved_ground_drop_settlement(
        &mut self,
        key: &ZonePresenceKey,
        ticket: &GroundDropClaimTicket,
        idempotency_key: String,
        execution_context: Option<SharedAccountInventoryExecutionContext>,
    ) -> bool {
        let Some(session_id) = self.zone_sessions.get(key).cloned() else {
            return false;
        };
        if &ticket.session_id != &session_id {
            return false;
        }
        let Some(zone_key) = self.zone_manager.zone_key_for_session(&session_id) else {
            return false;
        };
        let settlement = UnresolvedGroundDropSettlement {
            idempotency_key: Some(idempotency_key),
            execution_context,
            presence_key: key.clone(),
            zone_key,
            ticket: ticket.clone(),
        };
        let recovery_key = settlement.recovery_key();
        match self.unresolved_ground_drop_settlements.get(&recovery_key) {
            Some(existing) => return existing == &settlement,
            None => {}
        }
        if self
            .zone_manager
            .detach_ground_drop_claim(&session_id, ticket)
            .as_ref()
            != Some(&settlement.zone_key)
        {
            return false;
        }
        self.unresolved_ground_drop_settlements
            .insert(recovery_key, settlement);
        true
    }

    fn unresolved_ground_drop_settlement_for_presence(
        &self,
        key: &ZonePresenceKey,
    ) -> Option<UnresolvedGroundDropSettlement> {
        self.unresolved_ground_drop_settlements
            .values()
            .find(|settlement| settlement.involves(key))
            .cloned()
    }

    fn retain_unresolved_ground_drop_outcome_key(
        &mut self,
        expected: &UnresolvedGroundDropSettlement,
        idempotency_key: String,
    ) -> bool {
        let recovery_key = expected.recovery_key();
        let Some(stored) = self
            .unresolved_ground_drop_settlements
            .get_mut(&recovery_key)
        else {
            return false;
        };
        if stored != expected {
            return false;
        }
        match stored.idempotency_key.as_ref() {
            Some(existing) => existing == &idempotency_key,
            None => {
                stored.idempotency_key = Some(idempotency_key);
                true
            }
        }
    }

    fn resolve_unresolved_ground_drop_settlement(
        &mut self,
        expected: &UnresolvedGroundDropSettlement,
        committed: bool,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let recovery_key = expected.recovery_key();
        if self.unresolved_ground_drop_settlements.get(&recovery_key) != Some(expected) {
            return None;
        }
        let outbounds = if committed {
            Vec::new()
        } else {
            let outbounds = self.zone_manager.restore_detached_ground_drop_claim(
                &expected.zone_key,
                &expected.ticket,
                now_ms,
            )?;
            self.restore_drop(
                &expected.zone_key.map_file_name,
                expected.ticket.drop.clone(),
            );
            outbounds
        };
        self.unresolved_ground_drop_settlements
            .remove(&recovery_key);
        Some(outbounds)
    }

    fn take_pending_zone_ground_drop_claims(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<GroundDropClaimTicket> {
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

    fn sync_authoritative_zone_drops_for_key(
        &mut self,
        key: &ZonePresenceKey,
        drops: &[GroundDropSnapshot],
    ) {
        if drops.is_empty() {
            return;
        }
        let Some(map_file_name) = self
            .players
            .get(key)
            .map(|presence| presence.map_file_name.clone())
        else {
            return;
        };
        let now_ms = shared_gateway_now_ms();
        let map = self.maps.entry(map_file_name).or_default();
        for drop in drops {
            if map.removed_drop_ids.contains(&drop.object_id) {
                continue;
            }
            Self::sync_drop_ownership_deadline(map, drop, now_ms);
            // ObjectItem/ObjectGold packets only contain the legacy client-facing
            // fields. The kill award carries the authoritative item, ownership,
            // quantity, and monster provenance used by persistence and pickup.
            map.ground_drops.insert(drop.object_id, drop.clone());
        }
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
        Vec<GroundDropClaimTicket>,
        Vec<ZoneMonsterKillAward>,
        Vec<i32>,
        Vec<i32>,
    ) {
        self.dispatch_zone_outbounds_with_fence_policy(outbounds, current_key, false)
    }

    fn dispatch_zone_outbounds_with_fence_policy(
        &mut self,
        outbounds: Vec<ZoneOutbound>,
        current_key: Option<&ZonePresenceKey>,
        allow_fenced_current: bool,
    ) -> (
        Vec<ServerPacket>,
        Option<(Point, MirDirection)>,
        Option<(bool, bool)>,
        Vec<GroundDropClaimTicket>,
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
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
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
                        if self.teardown_fenced(&key)
                            && !(allow_fenced_current && current_key == Some(&key))
                        {
                            continue;
                        }
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
                        if self.teardown_fenced(&key)
                            && !(allow_fenced_current && current_key == Some(&key))
                        {
                            continue;
                        }
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
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
                    self.update_player_transform(&key, position.clone(), direction);
                    if current_key == Some(&key) {
                        current_transform = Some((position, direction));
                    } else {
                        self.pending_zone_transforms
                            .insert(key, (position, direction));
                    }
                }
                ZoneOutbound::NpcTeleportCommit { .. } => {
                    // Consumed atomically by execute_zone_npc_teleport before
                    // ordinary outbound dispatch. It must never reach a socket.
                }
                ZoneOutbound::ConsumeShoutPermission {
                    session_id,
                    map_shout,
                    server_shout,
                } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
                    if current_key == Some(&key) {
                        current_shout_consume = Some((map_shout, server_shout));
                    } else {
                        self.queue_zone_shout_consume(key, map_shout, server_shout);
                    }
                }
                // Legacy object-id-only claims are intentionally ignored. A
                // modern Zone always emits the authority-bearing ticket form.
                ZoneOutbound::GroundDropClaimed { .. } => {}
                ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
                    self.remove_shared_drop_for_key(&key, ticket.object_id);
                    if current_key == Some(&key) {
                        current_ground_drop_claims.push(ticket);
                    } else {
                        self.queue_zone_ground_drop_claim(key, ticket);
                    }
                }
                ZoneOutbound::MonsterKillAward { session_id, award } => {
                    let Some(key) = self.zone_session_keys.get(&session_id).cloned() else {
                        continue;
                    };
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
                    self.sync_authoritative_zone_drops_for_key(&key, &award.drops);
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
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
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
                    if self.teardown_fenced(&key)
                        && !(allow_fenced_current && current_key == Some(&key))
                    {
                        continue;
                    }
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
        // `SimulationSession` still owns personal quest/inventory state and its
        // compatibility ECS continues to tick. Once a monster has entered the
        // shared Zone, however, that private mirror must not move or heal the
        // public projection again. In particular, a later personal snapshot can
        // carry a different transform while the Zone-native monster is still
        // alive; clients then click the displayed tile and the Zone silently
        // rejects the attack against its object at another tile.
        //
        // Snapshot the single-writer Zone state before borrowing the map layer,
        // then re-apply its combat/transform fields after merging richer Crystal
        // metadata from the personal runtime.
        let native_monsters = self
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map(&map_file_name))
            .into_iter()
            .map(|monster| (monster.object_id, monster))
            .collect::<BTreeMap<_, _>>();
        let map = self.maps.entry(map_file_name).or_default();
        for mut entity in entities {
            let native_monster = (entity.kind == WorldEntityKind::Monster)
                .then(|| native_monsters.get(&entity.object_id))
                .flatten();
            if native_monster.is_some_and(|monster| !monster.dead && monster.hp > 0) {
                map.removed_entity_ids.remove(&entity.object_id);
            }
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
                let mut entity =
                    merge_shared_entity_state(map.entities.get(&entity.object_id), entity);
                if let Some(monster) = native_monster {
                    reconcile_shared_entity_with_native_monster(map, &mut entity, monster);
                }
                map.entities.insert(entity.object_id, entity);
            }
        }

        // A personal viewport can omit an already-shared monster. Reconcile the
        // retained map entry as well so absence from this particular snapshot
        // cannot leave a stale client-visible transform behind.
        for monster in native_monsters.values() {
            if monster.dead && map.removed_entity_ids.contains(&monster.object_id) {
                continue;
            }
            let Some(mut entity) = map.entities.remove(&monster.object_id) else {
                continue;
            };
            if entity.kind == WorldEntityKind::Monster {
                reconcile_shared_entity_with_native_monster(map, &mut entity, monster);
            }
            map.entities.insert(entity.object_id, entity);
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
        let native_monsters_by_object_id = self
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map(map_file_name))
            .into_iter()
            .map(|monster| (monster.object_id, monster))
            .collect::<BTreeMap<_, _>>();
        // `ObjectRemove` is also the Crystal AOI-leave packet. This method is
        // the final global map-index mutation boundary, so defend it directly
        // instead of relying on every caller to pre-classify observer-local
        // packets. A retained Zone object remains globally actionable even
        // while absent from one recipient's viewport.
        let zone_key = ZoneKey::for_map(map_file_name);
        let retained_remove_ids = self
            .zone_manager
            .zone(&zone_key)
            .map(|zone| {
                packets
                    .iter()
                    .filter_map(|packet| match packet {
                        ServerPacket::ObjectRemove { object_id }
                            if zone.retains_object_id(*object_id) =>
                        {
                            Some(*object_id)
                        }
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
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
                        // A live -> dead transition starts a new corpse
                        // incarnation. A retained Harvest marker belongs to an
                        // older incarnation and must not reject this corpse.
                        map.harvested_entity_ids.remove(&info.object_id);
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
                    let starts_new_corpse = map
                        .entities
                        .get(&info.object_id)
                        .is_some_and(|entity| !entity.dead && entity.hp.map_or(true, |hp| hp > 0));
                    if let Some(entity) = map.entities.get_mut(&info.object_id) {
                        entity.x = info.location.x;
                        entity.y = info.location.y;
                        entity.direction = info.direction;
                        entity.hp = Some(0);
                        entity.dead = true;
                    }
                    let follows_revive = map.revived_entity_ids.remove(&info.object_id);
                    if starts_new_corpse || follows_revive {
                        // Object ids are reused across Crystal monster
                        // incarnations. Clear an older completed-Harvest marker
                        // only on the authoritative live -> dead boundary; a
                        // duplicate late ObjectDied for the same corpse must not
                        // reopen an already harvested body.
                        map.harvested_entity_ids.remove(&info.object_id);
                    }
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
                    if retained_remove_ids.contains(object_id) {
                        continue;
                    }
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
                ServerPacket::ObjectItem { .. } | ServerPacket::ObjectGold { .. } => {
                    if let Some(drop) = ground_drop_snapshot_from_spawn_packet(packet) {
                        if !map.removed_drop_ids.contains(&drop.object_id) {
                            Self::sync_drop_ownership_deadline(map, &drop, shared_gateway_now_ms());
                            // A legacy spawn packet is intentionally lossy. Keep an
                            // authoritative snapshot already supplied by the Zone
                            // kill award instead of replacing it during the outer
                            // packet fan-out pass.
                            map.ground_drops.entry(drop.object_id).or_insert(drop);
                        }
                    }
                }
                ServerPacket::ObjectMonster { info } => {
                    if !map.removed_entity_ids.contains(&info.object_id) {
                        let mut entity = world_entity_from_monster_info(info);
                        entity.owner_name = player_names_by_zone_object_id
                            .get(&info.master_object_id)
                            .cloned();
                        if let Some(existing) = map.entities.get(&info.object_id) {
                            // ObjectMonster is a legacy client projection and
                            // does not carry authoritative level or health.
                            // Preserve richer finalized world-event metadata
                            // while applying its live transform/appearance.
                            entity.level = existing.level;
                            entity.hp = existing.hp;
                            entity.max_hp = existing.max_hp;
                            entity.quest_ids = existing.quest_ids.clone();
                            if entity.owner_name.is_none() {
                                entity.owner_name = existing.owner_name.clone();
                            }
                        }
                        // ObjectMonster has no relationship field. Rehydrate
                        // only from the live Zone authority; without one the
                        // legacy packet remains Neutral/fail-closed.
                        if let Some(monster) = native_monsters_by_object_id.get(&info.object_id) {
                            reconcile_shared_entity_with_native_monster(map, &mut entity, monster);
                        }
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

    fn filter_stale_dead_entity_packets(
        &self,
        map_file_name: &str,
        packets: &mut Vec<ServerPacket>,
    ) {
        let Some(map) = self.maps.get(map_file_name) else {
            return;
        };
        if map.dead_entity_ids.is_empty() {
            return;
        }
        packets.retain(|packet| {
            stale_dead_shared_entity_packet_object_id(packet)
                .is_none_or(|object_id| !map.dead_entity_ids.contains_key(&object_id))
        });
    }

    fn filter_stale_dead_entity_packets_for_key(
        &self,
        key: &ZonePresenceKey,
        packets: &mut Vec<ServerPacket>,
    ) {
        let Some(presence) = self.players.get(key) else {
            return;
        };
        self.filter_stale_dead_entity_packets(&presence.map_file_name, packets);
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
        if let Some(player) = self.players.values().find(|presence| {
            presence.map_file_name == map_file_name && presence.zone_object_id == object_id
        }) {
            return Some(player.entity.clone());
        }
        let map = self.maps.get(map_file_name)?;
        if map.removed_entity_ids.contains(&object_id) {
            return None;
        }
        map.entities.get(&object_id).cloned()
    }

    fn shared_player_pk_points(&self, map_file_name: &str, object_id: u32) -> Option<i32> {
        self.players
            .values()
            .find(|presence| {
                presence.map_file_name == map_file_name && presence.zone_object_id == object_id
            })
            .map(|presence| presence.pk_points)
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
        let points = shared_harvest_scan_points(picker, direction);
        let mut found_corpse = false;
        for point in points {
            for entity in map.entities.values().filter(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.x == point.x
                    && entity.y == point.y
                    && !map.removed_entity_ids.contains(&entity.object_id)
                    && (entity.dead || entity.hp.is_some_and(|hp| hp <= 0))
            }) {
                found_corpse = true;
                if !map.harvested_entity_ids.contains(&entity.object_id) {
                    return true;
                }
            }
        }
        !found_corpse
    }

    fn shared_harvest_target_snapshot(
        &self,
        map_file_name: &str,
        picker: &WorldEntitySnapshot,
        direction: MirDirection,
    ) -> Option<WorldEntitySnapshot> {
        let map = self.maps.get(map_file_name)?;
        for point in shared_harvest_scan_points(picker, direction) {
            if let Some(entity) = map.entities.values().find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.x == point.x
                    && entity.y == point.y
                    && !map.removed_entity_ids.contains(&entity.object_id)
                    && !map.harvested_entity_ids.contains(&entity.object_id)
                    && (entity.dead || entity.hp.is_some_and(|hp| hp <= 0))
            }) {
                return Some(entity.clone());
            }
        }
        None
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
        if let Some(map_file_name) = self
            .players
            .get(key)
            .map(|presence| presence.map_file_name.clone())
        {
            // Autonomous Zone ticks bypass the owning session runtime. Keep the
            // shared action index aligned with the movement/combat packets that
            // players receive, otherwise a world-event monster can move next to
            // a player while targeted actions still resolve against its spawn
            // position.
            // `ObjectRemove` is overloaded by the Crystal protocol: the Zone
            // emits it both when an object truly ceases to exist and when it
            // merely leaves one recipient's AOI. This map layer is the global
            // action index for the whole map, not that recipient's viewport.
            // Keep observer-local removes out of the global tombstone set while
            // the single-writer Zone still retains the object. Otherwise a
            // moving monster can reappear as a legacy projection with no native
            // combat target and soak attacks indefinitely.
            let zone_key = ZoneKey::for_map(&map_file_name);
            let visibility_only_remove_ids = self
                .zone_manager
                .zone(&zone_key)
                .map(|zone| {
                    packets
                        .iter()
                        .filter_map(|packet| match packet {
                            ServerPacket::ObjectRemove { object_id }
                                if zone.retains_object_id(*object_id) =>
                            {
                                Some(*object_id)
                            }
                            _ => None,
                        })
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            if visibility_only_remove_ids.is_empty() {
                self.apply_shared_entity_packets(&map_file_name, packets);
            } else {
                let global_packets = packets
                    .iter()
                    .filter(|packet| {
                        !matches!(
                            packet,
                            ServerPacket::ObjectRemove { object_id }
                                if visibility_only_remove_ids.contains(object_id)
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                self.apply_shared_entity_packets(&map_file_name, &global_packets);
            }
        }
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

    fn retain_unresolved_trade_settlement(
        &mut self,
        settlement: UnresolvedSharedTradeSettlement,
    ) -> Result<(), String> {
        let recovery_key = settlement.recovery_key();
        match self.unresolved_trade_settlements.get(&recovery_key) {
            Some(existing) if existing == &settlement => Ok(()),
            Some(_) => Err("conflicting unresolved trade settlement recovery key".to_string()),
            None => {
                self.unresolved_trade_settlements
                    .insert(recovery_key, settlement);
                Ok(())
            }
        }
    }

    fn unresolved_trade_settlement_for_presence(
        &self,
        key: &ZonePresenceKey,
    ) -> Option<UnresolvedSharedTradeSettlement> {
        self.unresolved_trade_settlements
            .values()
            .find(|settlement| settlement.involves(key))
            .cloned()
    }

    fn has_unresolved_trade_settlement_for_presence(&self, key: &ZonePresenceKey) -> bool {
        self.unresolved_trade_settlements
            .values()
            .any(|settlement| settlement.involves(key))
    }

    fn resolve_unresolved_trade_settlement(
        &mut self,
        expected: &UnresolvedSharedTradeSettlement,
        resolution: UnresolvedSharedTradeResolution,
    ) -> bool {
        let recovery_key = expected.recovery_key();
        if self.unresolved_trade_settlements.get(&recovery_key) != Some(expected) {
            return false;
        }
        self.unresolved_trade_settlements.remove(&recovery_key);
        match resolution {
            UnresolvedSharedTradeResolution::Durable => {}
            UnresolvedSharedTradeResolution::LocalCommit => {
                self.pending_trade_deliveries
                    .entry(expected.first_key.clone())
                    .or_default()
                    .push(expected.second_offer.clone());
                self.pending_trade_deliveries
                    .entry(expected.second_key.clone())
                    .or_default()
                    .push(expected.first_offer.clone());
            }
            UnresolvedSharedTradeResolution::Rejected => {
                self.pending_trade_rollbacks
                    .entry(expected.first_key.clone())
                    .or_default()
                    .push(expected.first_offer.clone());
                self.pending_trade_rollbacks
                    .entry(expected.second_key.clone())
                    .or_default()
                    .push(expected.second_offer.clone());
            }
        }
        true
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnresolvedSharedTradeResolution {
    Durable,
    LocalCommit,
    Rejected,
}

type SharedZoneMutationObserver =
    Arc<dyn Fn(&ZoneId, u64) -> Result<(), String> + Send + Sync + 'static>;
type SharedZoneTickAuthorizer = Arc<dyn Fn(&ZoneId) -> Result<(), String> + Send + Sync + 'static>;

#[derive(Debug, Default)]
struct SharedZoneMutationGateState {
    lanes: BTreeMap<ZoneId, SharedZoneMutationLane>,
}

#[derive(Debug, Default)]
struct SharedZoneMutationLane {
    next_ticket: u64,
    serving_ticket: u64,
    waiters: BTreeMap<u64, Arc<SharedZoneMutationWaiter>>,
}

#[derive(Debug, Default)]
struct SharedZoneMutationWaiter {
    ready: Mutex<bool>,
    changed: Condvar,
}

/// FIFO gate shared by player commands and autonomous Zone ticks.
///
/// `std::sync::Mutex` does not promise fair waiter selection. Under a sustained
/// multi-session journal load one RPC could therefore wait behind thousands of
/// later arrivals and eventually hit its socket timeout. Ticket ordering makes
/// the single-writer sequence explicit and bounded by the work already queued.
#[derive(Debug, Default)]
pub(crate) struct SharedZoneMutationGate {
    scope: RwLock<()>,
    state: Mutex<SharedZoneMutationGateState>,
}

impl SharedZoneMutationGate {
    /// Stops every Zone lane for host-wide checkpoint install/export.
    pub(crate) fn lock(&self) -> Result<SharedZoneMutationGateGuard<'_>, String> {
        let scope = self
            .scope
            .write()
            .map_err(|_| "shared Zone mutation scope was poisoned".to_string())?;
        Ok(SharedZoneMutationGateGuard {
            gate: self,
            zone_id: None,
            _scope: SharedZoneMutationScopeGuard::Exclusive { _guard: scope },
        })
    }

    /// Serializes one Zone while allowing unrelated maps to make progress.
    pub(crate) fn lock_zone(
        &self,
        zone_id: &ZoneId,
    ) -> Result<SharedZoneMutationGateGuard<'_>, String> {
        let scope = self
            .scope
            .read()
            .map_err(|_| "shared Zone mutation scope was poisoned".to_string())?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "shared Zone mutation gate mutex was poisoned".to_string())?;
        let lane = state.lanes.entry(zone_id.clone()).or_default();
        let ticket = lane.next_ticket;
        lane.next_ticket = lane.next_ticket.wrapping_add(1);
        if lane.serving_ticket == ticket {
            return Ok(SharedZoneMutationGateGuard {
                gate: self,
                zone_id: Some(zone_id.clone()),
                _scope: SharedZoneMutationScopeGuard::Shared { _guard: scope },
            });
        }

        let waiter = Arc::new(SharedZoneMutationWaiter::default());
        lane.waiters.insert(ticket, Arc::clone(&waiter));
        drop(state);

        let mut ready = waiter
            .ready
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !*ready {
            ready = waiter
                .changed
                .wait(ready)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        Ok(SharedZoneMutationGateGuard {
            gate: self,
            zone_id: Some(zone_id.clone()),
            _scope: SharedZoneMutationScopeGuard::Shared { _guard: scope },
        })
    }
}

enum SharedZoneMutationScopeGuard<'a> {
    Shared { _guard: RwLockReadGuard<'a, ()> },
    Exclusive { _guard: RwLockWriteGuard<'a, ()> },
}

pub(crate) struct SharedZoneMutationGateGuard<'a> {
    gate: &'a SharedZoneMutationGate,
    zone_id: Option<ZoneId>,
    _scope: SharedZoneMutationScopeGuard<'a>,
}

impl Drop for SharedZoneMutationGateGuard<'_> {
    fn drop(&mut self) {
        let Some(zone_id) = self.zone_id.as_ref() else {
            return;
        };
        let next = {
            let mut state = self
                .gate
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let lane = state
                .lanes
                .get_mut(zone_id)
                .expect("acquired Zone mutation lane should remain registered");
            lane.serving_ticket = lane.serving_ticket.wrapping_add(1);
            let serving_ticket = lane.serving_ticket;
            lane.waiters.remove(&serving_ticket)
        };
        if let Some(waiter) = next {
            *waiter
                .ready
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            waiter.changed.notify_one();
        }
    }
}

#[derive(Clone)]
struct SharedZoneMutationCapture {
    gate: Arc<SharedZoneMutationGate>,
    authorize_tick: SharedZoneTickAuthorizer,
    observer: SharedZoneMutationObserver,
}

impl fmt::Debug for SharedZoneMutationCapture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedZoneMutationCapture")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct SharedInProcessZoneRuntimeFactory {
    zones: Arc<Mutex<BTreeMap<ZoneId, SharedInProcessZoneResources>>>,
    account_inventory_service: SharedAccountInventoryServiceHandle,
    npc_world_service: SharedNpcWorldServiceHandle,
    default_tick_cadence: Duration,
    tick_cadences: Arc<BTreeMap<ZoneId, Duration>>,
    mutation_capture: Arc<Mutex<Option<SharedZoneMutationCapture>>>,
    autonomous_ticks_by_default: bool,
    replica_zone_ids: Arc<Mutex<BTreeSet<ZoneId>>>,
}

impl fmt::Debug for SharedInProcessZoneRuntimeFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedInProcessZoneRuntimeFactory")
            .field("active_zone_count", &self.active_zone_count())
            .field("default_tick_cadence", &self.default_tick_cadence)
            .field(
                "autonomous_ticks_by_default",
                &self.autonomous_ticks_by_default,
            )
            .finish_non_exhaustive()
    }
}

impl SharedInProcessZoneRuntimeFactory {
    pub fn new() -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service: Arc::new(InProcessAccountInventoryService::new()),
            npc_world_service: Arc::new(InProcessNpcWorldService),
            default_tick_cadence: Duration::from_millis(SHARED_CRYSTAL_TICK_MS),
            tick_cadences: Arc::new(BTreeMap::new()),
            mutation_capture: Arc::new(Mutex::new(None)),
            autonomous_ticks_by_default: true,
            replica_zone_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn with_tick_cadences(
        default_tick_cadence: Duration,
        tick_cadences: BTreeMap<ZoneId, Duration>,
    ) -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service: Arc::new(InProcessAccountInventoryService::new()),
            npc_world_service: Arc::new(InProcessNpcWorldService),
            default_tick_cadence: default_tick_cadence.max(Duration::from_millis(1)),
            tick_cadences: Arc::new(tick_cadences),
            mutation_capture: Arc::new(Mutex::new(None)),
            autonomous_ticks_by_default: true,
            replica_zone_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn with_tick_cadences_and_account_inventory_service(
        default_tick_cadence: Duration,
        tick_cadences: BTreeMap<ZoneId, Duration>,
        account_inventory_service: SharedAccountInventoryServiceHandle,
    ) -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service,
            npc_world_service: Arc::new(InProcessNpcWorldService),
            default_tick_cadence: default_tick_cadence.max(Duration::from_millis(1)),
            tick_cadences: Arc::new(tick_cadences),
            mutation_capture: Arc::new(Mutex::new(None)),
            autonomous_ticks_by_default: true,
            replica_zone_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn fresh(&self) -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service: self.account_inventory_service.clone(),
            npc_world_service: self.npc_world_service.clone(),
            default_tick_cadence: self.default_tick_cadence,
            tick_cadences: self.tick_cadences.clone(),
            mutation_capture: self.mutation_capture.clone(),
            autonomous_ticks_by_default: self.autonomous_ticks_by_default,
            replica_zone_ids: Arc::new(Mutex::new(
                self.replica_zone_ids
                    .lock()
                    .expect("shared Zone replica marker mutex should not be poisoned")
                    .clone(),
            )),
        }
    }

    pub(crate) fn fresh_replica(&self) -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service: self.account_inventory_service.clone(),
            npc_world_service: self.npc_world_service.clone(),
            default_tick_cadence: self.default_tick_cadence,
            tick_cadences: self.tick_cadences.clone(),
            mutation_capture: self.mutation_capture.clone(),
            autonomous_ticks_by_default: false,
            replica_zone_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub(crate) fn configure_mutation_capture(
        &self,
        gate: Arc<SharedZoneMutationGate>,
        authorize_tick: SharedZoneTickAuthorizer,
        observer: SharedZoneMutationObserver,
    ) {
        *self
            .mutation_capture
            .lock()
            .expect("shared Zone mutation capture mutex should not be poisoned") =
            Some(SharedZoneMutationCapture {
                gate,
                authorize_tick,
                observer,
            });
    }

    pub(crate) fn mark_zone_as_replica(&self, zone_id: &ZoneId) {
        // Fixed lock order for operations that need both maps: replicas, then Zones.
        let mut replica_zone_ids = self
            .replica_zone_ids
            .lock()
            .expect("shared Zone replica marker mutex should not be poisoned");
        let zones = self
            .zones
            .lock()
            .expect("shared Zone factory mutex should not be poisoned");
        replica_zone_ids.insert(zone_id.clone());
        if let Some(resources) = zones.get(zone_id) {
            resources
                .autonomous_ticks_enabled
                .store(false, Ordering::Release);
        }
    }

    pub(crate) fn is_zone_replica(&self, zone_id: &ZoneId) -> bool {
        self.replica_zone_ids
            .lock()
            .map(|zone_ids| zone_ids.contains(zone_id))
            .unwrap_or(false)
    }

    pub(crate) fn promote_zone_from_replica(&self, zone_id: &ZoneId) -> Result<(), String> {
        // Keep the same replicas -> Zones lock order as mark/restore.
        let mut replica_zone_ids = self
            .replica_zone_ids
            .lock()
            .map_err(|_| "shared Zone replica marker mutex was poisoned".to_string())?;
        let zones = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?;
        if !replica_zone_ids.contains(zone_id) {
            return Err(format!("Zone {zone_id} is not a standby replica"));
        }
        let resources = zones
            .get(zone_id)
            .cloned()
            .ok_or_else(|| format!("Zone {zone_id} has no installed replica state"))?;
        replica_zone_ids.remove(zone_id);
        resources
            .autonomous_ticks_enabled
            .store(true, Ordering::Release);
        Ok(())
    }

    pub(crate) fn quiesce_active_zone(&self, zone_id: &ZoneId) -> Result<(), String> {
        if self.is_zone_replica(zone_id) {
            return Err(format!("Zone {zone_id} is a standby replica"));
        }
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .get(zone_id)
            .cloned()
            .ok_or_else(|| format!("Zone {zone_id} has no active runtime state"))?;
        resources
            .autonomous_ticks_enabled
            .store(false, Ordering::Release);
        Ok(())
    }

    pub(crate) fn resume_active_zone(&self, zone_id: &ZoneId) -> Result<(), String> {
        // Keep the replica check and tick enable in the same replicas -> Zones
        // critical section as mark/promote/restore/resource creation.
        let replica_zone_ids = self
            .replica_zone_ids
            .lock()
            .map_err(|_| "shared Zone replica marker mutex was poisoned".to_string())?;
        let zones = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?;
        if replica_zone_ids.contains(zone_id) {
            return Err(format!("Zone {zone_id} is a standby replica"));
        }
        let resources = zones
            .get(zone_id)
            .cloned()
            .ok_or_else(|| format!("Zone {zone_id} has no active runtime state"))?;
        resources
            .autonomous_ticks_enabled
            .store(true, Ordering::Release);
        Ok(())
    }

    pub(crate) fn autonomous_ticks_enabled(&self, zone_id: &ZoneId) -> bool {
        self.zones
            .lock()
            .ok()
            .and_then(|zones| zones.get(zone_id).cloned())
            .is_some_and(|resources| resources.autonomous_ticks_enabled.load(Ordering::Acquire))
    }

    pub fn checkpoint_bytes(&self) -> Result<Vec<u8>, String> {
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .clone();
        let mut zones = BTreeMap::new();
        for (zone_id, resources) in resources {
            let checkpoint = resources
                .zone_state
                .lock()
                .map_err(|_| format!("shared Zone {} state mutex was poisoned", zone_id))?
                .checkpoint()?;
            zones.insert(zone_id, checkpoint);
        }
        serde_json::to_vec(&SharedInProcessZoneFactoryCheckpoint {
            version: SHARED_ZONE_FACTORY_CHECKPOINT_VERSION,
            zones,
        })
        .map_err(|error| format!("failed to encode shared Zone factory checkpoint: {error}"))
    }

    /// Persist only process-independent world state. Gateway/Zone sessions and
    /// their transient outbound queues must be recreated by live clients.
    pub(crate) fn world_checkpoint_bytes(&self) -> Result<Vec<u8>, String> {
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .clone();
        let mut zones = BTreeMap::new();
        for (zone_id, resources) in resources {
            let checkpoint = resources
                .zone_state
                .lock()
                .map_err(|_| format!("shared Zone {} state mutex was poisoned", zone_id))?
                .world_checkpoint()?;
            zones.insert(zone_id, checkpoint);
        }
        serde_json::to_vec(&SharedInProcessZoneFactoryCheckpoint {
            version: SHARED_ZONE_FACTORY_CHECKPOINT_VERSION,
            zones,
        })
        .map_err(|error| format!("failed to encode shared Zone world checkpoint: {error}"))
    }

    pub fn zone_checkpoint_bytes(&self, zone_id: &ZoneId) -> Result<Vec<u8>, String> {
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .get(zone_id)
            .cloned();
        let mut zones = BTreeMap::new();
        if let Some(resources) = resources {
            let checkpoint = resources
                .zone_state
                .lock()
                .map_err(|_| format!("shared Zone {} state mutex was poisoned", zone_id))?
                .checkpoint()?;
            zones.insert(zone_id.clone(), checkpoint);
        }
        serde_json::to_vec(&SharedInProcessZoneFactoryCheckpoint {
            version: SHARED_ZONE_FACTORY_CHECKPOINT_VERSION,
            zones,
        })
        .map_err(|error| {
            format!(
                "failed to encode shared Zone {} checkpoint: {error}",
                zone_id
            )
        })
    }

    pub fn install_checkpoint_bytes(&self, bytes: &[u8]) -> Result<usize, String> {
        let checkpoint: SharedInProcessZoneFactoryCheckpoint = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode shared Zone factory checkpoint: {error}"))?;
        self.install_factory_checkpoint(checkpoint)
    }

    /// Preserve the existing caller API while using the transactional
    /// implementation for every World Director restore.
    pub(crate) fn install_world_checkpoint_bytes(&self, bytes: &[u8]) -> Result<usize, String> {
        self.install_world_checkpoint_bytes_atomically(bytes)
    }

    /// Restore a complete World Director image without exposing a partially
    /// restored set of Zones. Every Zone is decoded and restored into an
    /// isolated resource map before the live map is replaced under one lock.
    pub(crate) fn install_world_checkpoint_bytes_atomically(
        &self,
        bytes: &[u8],
    ) -> Result<usize, String> {
        let mut checkpoint: SharedInProcessZoneFactoryCheckpoint = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode shared Zone factory checkpoint: {error}"))?;
        checkpoint.zones = checkpoint
            .zones
            .into_iter()
            .map(|(zone_id, zone_checkpoint)| {
                Ok((zone_id, zone_checkpoint.into_verified_world_only()?))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        self.install_factory_checkpoint_atomically(checkpoint)
    }

    fn install_factory_checkpoint_atomically(
        &self,
        checkpoint: SharedInProcessZoneFactoryCheckpoint,
    ) -> Result<usize, String> {
        if checkpoint.version != SHARED_ZONE_FACTORY_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported shared Zone factory checkpoint version {}, expected {}",
                checkpoint.version, SHARED_ZONE_FACTORY_CHECKPOINT_VERSION
            ));
        }

        let zone_count = checkpoint.zones.len();
        let mut restored_states = BTreeMap::new();
        for (zone_id, zone_checkpoint) in checkpoint.zones {
            restored_states.insert(zone_id, SharedInProcessZoneState::restore(zone_checkpoint)?);
        }

        let mut staged_resources = BTreeMap::new();
        for (zone_id, restored) in restored_states {
            let cadence = self
                .tick_cadences
                .get(&zone_id)
                .copied()
                .unwrap_or(self.default_tick_cadence);
            let resources = SharedInProcessZoneResources::new(
                &zone_id,
                cadence,
                self.mutation_capture.clone(),
                false,
            );
            *resources
                .zone_state
                .lock()
                .map_err(|_| format!("staged shared Zone {zone_id} state mutex was poisoned"))? =
                restored;
            staged_resources.insert(zone_id, resources);
        }

        let checkpoint_zone_ids = staged_resources.keys().cloned().collect::<BTreeSet<_>>();
        // One atomic critical section observes replica markers, rechecks the
        // live Zone set, replaces it, and initializes every tick flag. The
        // fixed order is replicas -> Zones, matching mark/promote.
        let replica_zone_ids = self
            .replica_zone_ids
            .lock()
            .map_err(|_| "shared Zone replica marker mutex was poisoned".to_string())?;
        let mut live_resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?;
        let current_zone_ids = live_resources.keys().cloned().collect::<BTreeSet<_>>();
        if !current_zone_ids.is_subset(&checkpoint_zone_ids) {
            return Err(
                "shared Zone checkpoint is missing a Zone created during journal replay"
                    .to_string(),
            );
        }

        *live_resources = staged_resources;
        for (zone_id, resources) in live_resources.iter() {
            let autonomous_ticks_enabled =
                self.autonomous_ticks_by_default && !replica_zone_ids.contains(zone_id);
            resources
                .autonomous_ticks_enabled
                .store(autonomous_ticks_enabled, Ordering::Release);
        }
        Ok(zone_count)
    }

    fn install_factory_checkpoint(
        &self,
        checkpoint: SharedInProcessZoneFactoryCheckpoint,
    ) -> Result<usize, String> {
        if checkpoint.version != SHARED_ZONE_FACTORY_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported shared Zone factory checkpoint version {}, expected {}",
                checkpoint.version, SHARED_ZONE_FACTORY_CHECKPOINT_VERSION
            ));
        }
        let checkpoint_zone_ids = checkpoint.zones.keys().cloned().collect::<BTreeSet<_>>();
        let current_zone_ids = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !current_zone_ids.is_subset(&checkpoint_zone_ids) {
            return Err(
                "shared Zone checkpoint is missing a Zone created during journal replay"
                    .to_string(),
            );
        }

        let zone_count = checkpoint.zones.len();
        for (zone_id, zone_checkpoint) in checkpoint.zones {
            let restored = SharedInProcessZoneState::restore(zone_checkpoint)?;
            let resources = self.resources_for_zone(&zone_id);
            *resources
                .zone_state
                .lock()
                .map_err(|_| format!("shared Zone {} state mutex was poisoned", zone_id))? =
                restored;
        }
        Ok(zone_count)
    }

    /// Atomically publish one fully validated Zone resource image from an
    /// isolated factory. Existing resources for every other Zone are retained.
    pub fn adopt_zone_resources_from(
        &self,
        source: &SharedInProcessZoneRuntimeFactory,
        zone_id: &ZoneId,
    ) -> Result<bool, String> {
        if Arc::ptr_eq(&self.zones, &source.zones) {
            return Ok(self
                .zones
                .lock()
                .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
                .contains_key(zone_id));
        }
        let resources = source
            .zones
            .lock()
            .map_err(|_| "source shared Zone factory mutex was poisoned".to_string())?
            .remove(zone_id);
        let Some(resources) = resources else {
            return Ok(false);
        };
        self.zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .insert(zone_id.clone(), resources);
        Ok(true)
    }

    pub fn with_account_inventory_service(
        account_inventory_service: SharedAccountInventoryServiceHandle,
    ) -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service,
            npc_world_service: Arc::new(InProcessNpcWorldService),
            default_tick_cadence: Duration::from_millis(SHARED_CRYSTAL_TICK_MS),
            tick_cadences: Arc::new(BTreeMap::new()),
            mutation_capture: Arc::new(Mutex::new(None)),
            autonomous_ticks_by_default: true,
            replica_zone_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn with_world_services(
        account_inventory_service: SharedAccountInventoryServiceHandle,
        npc_world_service: SharedNpcWorldServiceHandle,
    ) -> Self {
        Self {
            zones: Arc::new(Mutex::new(BTreeMap::new())),
            account_inventory_service,
            npc_world_service,
            default_tick_cadence: Duration::from_millis(SHARED_CRYSTAL_TICK_MS),
            tick_cadences: Arc::new(BTreeMap::new()),
            mutation_capture: Arc::new(Mutex::new(None)),
            autonomous_ticks_by_default: true,
            replica_zone_ids: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    fn resources_for_zone(&self, zone_id: &ZoneId) -> SharedInProcessZoneResources {
        let cadence = self
            .tick_cadences
            .get(zone_id)
            .copied()
            .unwrap_or(self.default_tick_cadence);
        // Keep replica observation and first resource insertion in the same
        // replicas -> Zones critical section as mark/promote/restore. Otherwise
        // a concurrent mark could install the marker between the two locks and
        // leave a standby resource with autonomous ticks enabled.
        let replica_zone_ids = self
            .replica_zone_ids
            .lock()
            .expect("shared Zone replica marker mutex should not be poisoned");
        let autonomous_ticks_enabled =
            self.autonomous_ticks_by_default && !replica_zone_ids.contains(zone_id);
        let mut zones = self
            .zones
            .lock()
            .expect("shared zone factory mutex should not be poisoned");
        zones
            .entry(zone_id.clone())
            .or_insert_with(|| {
                SharedInProcessZoneResources::new(
                    zone_id,
                    cadence,
                    self.mutation_capture.clone(),
                    autonomous_ticks_enabled,
                )
            })
            .clone()
    }

    pub fn active_zone_count(&self) -> usize {
        self.zones
            .lock()
            .map(|zones| zones.len())
            .unwrap_or_default()
    }

    pub fn zone_tick_count(&self, zone_id: &ZoneId) -> u64 {
        self.zones
            .lock()
            .ok()
            .and_then(|zones| zones.get(zone_id).cloned())
            .map(|resources| resources.tick_count.load(Ordering::Acquire))
            .unwrap_or_default()
    }

    /// Apply finalized world-director monster spawns through the same per-Zone
    /// single-writer gate and replication observer used by autonomous ticks.
    pub fn apply_world_event_monsters(
        &self,
        zone_id: &ZoneId,
        map_file_name: &str,
        spawns: &[ZoneMonsterSpawn],
        now_ms: u64,
    ) -> Result<usize, String> {
        if map_file_name.trim().is_empty() {
            return Err("world event map file name must not be empty".to_string());
        }
        let resources = self.resources_for_zone(zone_id);
        let capture = self
            .mutation_capture
            .lock()
            .map_err(|_| "shared Zone mutation capture mutex was poisoned".to_string())?
            .clone();
        let _gate = match capture.as_ref() {
            Some(capture) => Some(capture.gate.lock_zone(zone_id)?),
            None => None,
        };
        if let Some(capture) = capture.as_ref() {
            (capture.authorize_tick)(zone_id)?;
        }

        let mut zone_state = resources
            .zone_state
            .lock()
            .map_err(|_| format!("shared Zone {zone_id} state mutex was poisoned"))?;
        let key = ZoneKey::for_map(map_file_name);
        let mut spawned = 0;
        for spawn in spawns {
            let (accepted, outbounds) =
                zone_state
                    .zone_manager
                    .spawn_world_event_monster(key.clone(), spawn, now_ms);
            if accepted {
                spawned += 1;
            }
            if let Some(monster) = zone_state
                .zone_manager
                .native_monster_snapshots(&key)
                .into_iter()
                .find(|monster| monster.object_id == spawn.object_id)
            {
                let map = zone_state
                    .maps
                    .entry(map_file_name.to_string())
                    .or_default();
                map.entities
                    .entry(spawn.object_id)
                    .or_insert_with(|| world_entity_from_zone_monster_spawn(spawn, &monster));
            }
            let _ = zone_state.dispatch_zone_outbounds(outbounds, None);
        }
        drop(zone_state);
        if let Some(capture) = capture.as_ref() {
            (capture.observer)(zone_id, now_ms)?;
        }
        Ok(spawned)
    }

    pub fn broadcast_world_event_message(
        &self,
        zone_id: &ZoneId,
        map_file_name: &str,
        message: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if message.trim().is_empty() {
            return Err("world event message must not be empty".to_string());
        }
        let resources = self.resources_for_zone(zone_id);
        let capture = self
            .mutation_capture
            .lock()
            .map_err(|_| "shared Zone mutation capture mutex was poisoned".to_string())?
            .clone();
        let _gate = match capture.as_ref() {
            Some(capture) => Some(capture.gate.lock_zone(zone_id)?),
            None => None,
        };
        if let Some(capture) = capture.as_ref() {
            (capture.authorize_tick)(zone_id)?;
        }
        let mut zone_state = resources
            .zone_state
            .lock()
            .map_err(|_| format!("shared Zone {zone_id} state mutex was poisoned"))?;
        let outbounds = zone_state
            .zone_manager
            .broadcast_world_event_message(ZoneKey::for_map(map_file_name), message);
        let _ = zone_state.dispatch_zone_outbounds(outbounds, None);
        drop(zone_state);
        if let Some(capture) = capture.as_ref() {
            (capture.observer)(zone_id, now_ms)?;
        }
        Ok(())
    }

    pub fn world_event_monster_count(
        &self,
        zone_id: &ZoneId,
        map_file_name: &str,
    ) -> Result<usize, String> {
        let resources = self.resources_for_zone(zone_id);
        let zone_state = resources
            .zone_state
            .lock()
            .map_err(|_| format!("shared Zone {zone_id} state mutex was poisoned"))?;
        Ok(zone_state
            .zone_manager
            .zone(&ZoneKey::for_map(map_file_name))
            .map(|runtime| runtime.native_monster_count())
            .unwrap_or_default())
    }

    pub fn world_event_monster_snapshots(
        &self,
        zone_id: &ZoneId,
        map_file_name: &str,
    ) -> Result<Vec<ZoneNativeMonsterSnapshot>, String> {
        let resources = self.resources_for_zone(zone_id);
        let zone_state = resources
            .zone_state
            .lock()
            .map_err(|_| format!("shared Zone {zone_id} state mutex was poisoned"))?;
        Ok(zone_state
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map(map_file_name)))
    }

    pub(crate) fn apply_replicated_zone_tick(
        &self,
        zone_id: &ZoneId,
        now_ms: u64,
    ) -> Result<(), String> {
        let resources = self.resources_for_zone(zone_id);
        run_shared_zone_cadence_tick(&resources.zone_state, now_ms)?;
        resources.tick_count.fetch_add(1, Ordering::Release);
        Ok(())
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
    if incoming.ai.is_none() {
        incoming.ai = existing.ai;
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

fn reconcile_shared_entity_with_native_monster(
    map: &mut ZoneMapSnapshotLayer,
    entity: &mut WorldEntitySnapshot,
    monster: &ZoneNativeMonsterSnapshot,
) {
    let was_dead = entity.dead
        || entity.hp.is_some_and(|hp| hp <= 0)
        || map.dead_entity_ids.contains_key(&monster.object_id);
    entity.x = monster.position.x;
    entity.y = monster.position.y;
    entity.hp = Some(monster.hp.max(0));
    entity.max_hp = Some(monster.max_hp.max(1));
    entity.dead = monster.dead || monster.hp <= 0;
    entity.disposition = monster
        .disposition
        .unwrap_or(WorldEntityDisposition::Neutral);

    if entity.dead {
        entity.hp = Some(0);
        map.revived_entity_ids.remove(&monster.object_id);
        map.dead_entity_ids.insert(
            monster.object_id,
            SharedDeadEntityState {
                location: Some(monster.position.clone()),
                direction: Some(entity.direction),
            },
        );
        return;
    }

    map.removed_entity_ids.remove(&monster.object_id);
    map.dead_entity_ids.remove(&monster.object_id);
    // A live Zone-native monster can never still be harvested. Keep the shared
    // action index self-healing even when the explicit ObjectRevived fan-out was
    // interleaved with a snapshot merge.
    map.harvested_entity_ids.remove(&monster.object_id);
    if was_dead {
        // Normally ObjectRevived already performs these transitions. Keep the
        // map self-healing if a snapshot is read between the single-writer
        // state change and its fan-out packet, without treating an arbitrary
        // legacy ObjectMonster packet as a revive.
        map.revived_entity_ids.insert(monster.object_id);
        map.committed_death_drop_anchors.remove(&monster.object_id);
    }
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
        ai: Some(info.ai),
        x: info.location.x,
        y: info.location.y,
        direction: info.direction,
        class: None,
        gender: None,
        level: None,
        riding_mount: None,
        can_mount_attack: None,
        has_class_weapon: None,
        dazed: None,
        fishing: None,
        hp: None,
        max_hp: None,
        light: info.light,
        name_colour_argb: info.name_colour_argb,
        dead: info.dead,
        // ObjectMonster carries behaviour AI but no authoritative relationship.
        // Keep legacy/missing producers fail-closed until a retained native
        // snapshot supplies explicit disposition.
        disposition: WorldEntityDisposition::Neutral,
        sprite: Some(simple_world_entity_sprite_snapshot(
            "Monster", info.image, 3,
        )),
        quest_ids: Vec::new(),
        quest_icon: None,
    }
}

fn world_entity_from_zone_monster_spawn(
    spawn: &ZoneMonsterSpawn,
    monster: &ZoneNativeMonsterSnapshot,
) -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id: monster.object_id,
        kind: WorldEntityKind::Monster,
        name: monster.name.clone(),
        owner_name: None,
        ai: Some(spawn.ai),
        x: monster.position.x,
        y: monster.position.y,
        direction: spawn.direction,
        class: None,
        gender: None,
        level: Some(spawn.level),
        riding_mount: None,
        can_mount_attack: None,
        has_class_weapon: None,
        dazed: None,
        fishing: None,
        hp: Some(monster.hp),
        max_hp: Some(monster.max_hp),
        light: 0,
        name_colour_argb: spawn.name_colour_argb,
        dead: monster.dead,
        disposition: monster
            .disposition
            .unwrap_or(WorldEntityDisposition::Neutral),
        sprite: Some(simple_world_entity_sprite_snapshot(
            "Monster",
            spawn.image,
            3,
        )),
        quest_ids: Vec::new(),
        quest_icon: None,
    }
}

fn fallback_monster_ai_for_shared_entity(entity: &WorldEntitySnapshot) -> u8 {
    match entity.disposition {
        WorldEntityDisposition::Friendly | WorldEntityDisposition::Neutral => 1,
        WorldEntityDisposition::Hostile => 0,
    }
}

fn zone_monster_ai_for_shared_entity(entity: &WorldEntitySnapshot, template_ai: Option<u8>) -> u8 {
    match (entity.disposition, template_ai) {
        (WorldEntityDisposition::Hostile, Some(1 | 2 | 3)) => {
            fallback_monster_ai_for_shared_entity(entity)
        }
        (_, Some(ai)) => ai,
        _ => fallback_monster_ai_for_shared_entity(entity),
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
        ai: zone_monster_ai_for_shared_entity(
            entity,
            entity.ai.or_else(|| template_ref.map(|monster| monster.ai)),
        ),
        disposition: Some(entity.disposition),
        level: entity
            .level
            .or_else(|| template_ref.map(|monster| monster.level))
            .unwrap_or(1),
        max_hp,
        hp,
        experience: template_ref.map(|monster| monster.experience).unwrap_or(0),
        move_speed_ms: template_ref
            .map(|monster| u64::from(monster.move_speed))
            .unwrap_or_default(),
        attack_speed_ms: template_ref
            .map(|monster| u64::from(monster.attack_speed))
            .unwrap_or_default(),
        friendly_guild: None,
        defense: template_ref
            .map(|monster| ZoneMonsterDefense::from_crystal_template(monster))
            .unwrap_or_default(),
        respawn: None,
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
        ai: None,
        x: info.location.x,
        y: info.location.y,
        direction: info.direction,
        class: Some(info.class),
        gender: Some(info.gender),
        level: Some(info.level),
        // Preserve the explicit remote appearance/presentation predicates;
        // combat-only predicates not present in ObjectPlayerInfo stay unknown.
        riding_mount: Some(info.riding_mount),
        can_mount_attack: None,
        has_class_weapon: None,
        dazed: None,
        fishing: Some(info.fishing),
        hp: None,
        max_hp: None,
        light: info.light,
        name_colour_argb: info.name_colour_argb,
        dead: info.dead,
        disposition: WorldEntityDisposition::Friendly,
        sprite: Some(world_entity_sprite_from_object_player(info)),
        quest_ids: Vec::new(),
        quest_icon: None,
    }
}

fn world_entity_from_npc_info(info: &NpcInfo) -> WorldEntitySnapshot {
    WorldEntitySnapshot {
        object_id: info.object_id,
        kind: WorldEntityKind::Npc,
        name: info.name.clone(),
        owner_name: None,
        ai: None,
        x: info.location.x,
        y: info.location.y,
        direction: info.direction,
        class: None,
        gender: None,
        level: None,
        riding_mount: None,
        can_mount_attack: None,
        has_class_weapon: None,
        dazed: None,
        fishing: None,
        hp: None,
        max_hp: None,
        light: 10,
        name_colour_argb: info.name_colour_argb,
        dead: false,
        disposition: WorldEntityDisposition::Neutral,
        sprite: Some(simple_world_entity_sprite_snapshot("NPC", info.image, 2)),
        quest_ids: info.quest_ids.clone(),
        quest_icon: None,
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
                ai: zone_monster_ai_for_shared_entity(
                    entity,
                    entity.ai.or_else(|| {
                        crystal_monster_by_name(&entity.name).map(|monster| monster.ai)
                    }),
                ),
                light: entity.light,
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
    let light = crystal_monster_by_name(&spawn.name)
        .map(|monster| monster.light)
        .unwrap_or(0);
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
            light,
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
const SHARED_ZONE_MOVEMENT_INGRESS_CAPACITY: usize = 64;
const SHARED_ZONE_MOVEMENT_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
const SHARED_ZONE_TRANSFER_FALLBACK_MARGIN: i32 = 2;
// Keep the WebSocket task free for chained Crystal movement after an ACK.
// Follow-up input is timed from the browser seeing the ACK, so add one Crystal
// tick of transport/render margin on top of the server-side run grace window.
const SHARED_ZONE_POST_MOVEMENT_INPUT_GRACE_MS: u64 = 1_500;

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

fn shared_harvest_scan_points(picker: &WorldEntitySnapshot, direction: MirDirection) -> Vec<Point> {
    let front = point_in_direction(
        &Point {
            x: picker.x,
            y: picker.y,
        },
        direction,
    );
    // Mirror SimulationSession::harvest_target_in_direction: first inspect the
    // facing cell, then the eight cells surrounding it. Crystal's skinning
    // reach is wider than a single exact coordinate, and the late death anchor
    // can legitimately differ by one movement cell from the rendered corpse.
    vec![
        front.clone(),
        Point {
            x: front.x - 1,
            y: front.y - 1,
        },
        Point {
            x: front.x,
            y: front.y - 1,
        },
        Point {
            x: front.x + 1,
            y: front.y - 1,
        },
        Point {
            x: front.x - 1,
            y: front.y,
        },
        Point {
            x: front.x + 1,
            y: front.y,
        },
        Point {
            x: front.x - 1,
            y: front.y + 1,
        },
        Point {
            x: front.x,
            y: front.y + 1,
        },
        Point {
            x: front.x + 1,
            y: front.y + 1,
        },
    ]
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
            | Spell::FireBang
            | Spell::IceStorm
            | Spell::Teleport
            | Spell::Blink
            | Spell::Blizzard
            | Spell::MeteorStrike
            | Spell::PoisonCloud
            | Spell::TrapHexagon
            | Spell::Curse
            | Spell::Plague
            | Spell::ExplosiveTrap
    )
}

fn gateway_zone_magic_targets_summon(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::Mirroring
            | Spell::SummonSkeleton
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
        Spell::Healing
            | Spell::Repulsion
            | Spell::EnergyRepulsor
            | Spell::HellFire
            | Spell::Lightning
            | Spell::ThunderStorm
            | Spell::FlameField
            | Spell::ShoulderDash
            | Spell::BladeAvalanche
            | Spell::LionRoar
            | Spell::ProtectionField
            | Spell::Rage
            | Spell::Fury
            | Spell::MagicBooster
            | Spell::Hiding
            | Spell::MassHiding
            | Spell::SoulShield
            | Spell::BlessedArmour
            | Spell::MassHealing
            | Spell::HealingCircle
            | Spell::Purification
            | Spell::EnergyShield
            | Spell::MagicShield
    )
}

fn gateway_zone_magic_requires_item_consumption(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::SoulFireBall
            | Spell::Hiding
            | Spell::SoulShield
            | Spell::BlessedArmour
            | Spell::UltimateEnhancer
            | Spell::Hallucination
            | Spell::Curse
            | Spell::TrapHexagon
            | Spell::Trap
            | Spell::DelayedExplosion
            | Spell::ExplosiveTrap
            | Spell::BindingShot
            | Spell::Poisoning
            | Spell::PoisonSword
            | Spell::PoisonShot
            | Spell::CrippleShot
            | Spell::PoisonCloud
            | Spell::Plague
            | Spell::Reincarnation
            | Spell::SummonSkeleton
            | Spell::SummonShinsu
            | Spell::SummonHolyDeva
    )
}

fn gateway_zone_magic_requires_item_preflight_only(spell: Spell) -> bool {
    matches!(spell, Spell::MassHiding)
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

fn ground_drop_snapshot_from_spawn_packet(packet: &ServerPacket) -> Option<GroundDropSnapshot> {
    match packet {
        ServerPacket::ObjectGold { info } => Some(GroundDropSnapshot {
            object_id: info.object_id,
            name: format!("{} Gold", info.gold),
            name_colour_argb: -1,
            icon: 0,
            x: info.location.x,
            y: info.location.y,
            quantity: info.gold.max(1),
            source_monster: String::new(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: info.gold },
        }),
        ServerPacket::ObjectItem { info } => Some(GroundDropSnapshot {
            object_id: info.object_id,
            name: info.name.clone(),
            name_colour_argb: info.name_colour_argb,
            icon: info.image,
            x: info.location.x,
            y: info.location.y,
            quantity: 1,
            source_monster: String::new(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::InventoryItem {
                key: info.name.clone(),
                name: info.name.clone(),
                description: String::new(),
                weight: 0,
                durability_current: None,
                durability_max: None,
                added_attack: 0,
                added_defence: 0,
                added_stats: Vec::new(),
                cursed: false,
                socket_slots: 0,
                show_group_pickup: false,
                exact_item: None,
            },
        }),
        _ => None,
    }
}

fn same_ground_drop_projection(first: &GroundDropSnapshot, second: &GroundDropSnapshot) -> bool {
    first.x == second.x
        && first.y == second.y
        && first.quantity == second.quantity
        && first.loot == second.loot
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

fn merge_ground_drop_claim_packets_in_crystal_order(
    packets: &mut Vec<ServerPacket>,
    mut claim_packets_by_object_id: BTreeMap<u32, Vec<ServerPacket>>,
    canceled_claims: &BTreeSet<u32>,
) {
    if claim_packets_by_object_id.is_empty() && canceled_claims.is_empty() {
        return;
    }

    let original_packets = std::mem::take(packets);
    let mut ordered_packets = Vec::with_capacity(
        original_packets.len()
            + claim_packets_by_object_id
                .values()
                .map(Vec::len)
                .sum::<usize>(),
    );
    for packet in original_packets {
        let removed_object_id = match &packet {
            ServerPacket::ObjectRemove { object_id } => Some(*object_id),
            _ => None,
        };
        if let Some(object_id) = removed_object_id {
            if let Some(claim_packets) = claim_packets_by_object_id.remove(&object_id) {
                // Crystal commits and reports the inventory gain before the
                // ground object is removed from the scene.
                ordered_packets.extend(claim_packets);
            }
            if canceled_claims.contains(&object_id) {
                continue;
            }
        }
        ordered_packets.push(packet);
    }
    for claim_packets in claim_packets_by_object_id.into_values() {
        ordered_packets.extend(claim_packets);
    }
    *packets = ordered_packets;
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

fn suppress_personal_tick_shared_monster_motion(
    packets: &mut Vec<ServerPacket>,
    shared_monster_ids: &BTreeSet<u32>,
) {
    packets.retain(|packet| {
        !coalesced_zone_movement_object_id(packet)
            .is_some_and(|object_id| shared_monster_ids.contains(&object_id))
    });
}

fn is_realtime_zone_live_packet(packet: &ServerPacket) -> bool {
    matches!(
        packet,
        ServerPacket::UserLocation { .. }
            | ServerPacket::ObjectPlayer { .. }
            | ServerPacket::ObjectTurn { .. }
            | ServerPacket::ObjectWalk { .. }
            | ServerPacket::ObjectRun { .. }
            | ServerPacket::ObjectRemove { .. }
    )
}

fn packets_include_user_location(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. }))
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

fn stale_dead_shared_entity_packet_object_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement }
        | ServerPacket::ObjectBackStep { movement, .. }
        | ServerPacket::ObjectSitDown { movement, .. } => Some(movement.object_id),
        ServerPacket::ObjectDash { object_id, .. }
        | ServerPacket::ObjectDashFail { object_id, .. }
        | ServerPacket::ObjectDashAttack { object_id, .. }
        | ServerPacket::ObjectPushed { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectAttack { info } => Some(info.object_id),
        ServerPacket::ObjectRangeAttack { info } => Some(info.object_id),
        ServerPacket::ObjectMagic { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectProjectile { source_id, .. } => Some(*source_id),
        ServerPacket::ObjectHealth { info } if info.percent > 0 => Some(info.object_id),
        ServerPacket::ObjectMana { info } => Some(info.object_id),
        _ => None,
    }
}

fn owner_dead_entity_marker_object_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectHealth { info } if info.percent == 0 => Some(info.object_id),
        ServerPacket::ObjectDied { info } => Some(info.object_id),
        ServerPacket::ObjectMonster { info } if info.dead => Some(info.object_id),
        ServerPacket::ObjectPlayer { info } if info.dead => Some(info.object_id),
        ServerPacket::ObjectHero { info, .. } if info.dead => Some(info.object_id),
        _ => None,
    }
}

fn owner_alive_entity_marker_object_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectRevived { info } => Some(info.object_id),
        ServerPacket::ObjectRemove { object_id } => Some(*object_id),
        ServerPacket::ObjectMonster { info } if !info.dead => Some(info.object_id),
        ServerPacket::ObjectPlayer { info } if !info.dead => Some(info.object_id),
        ServerPacket::ObjectHero { info, .. } if !info.dead => Some(info.object_id),
        _ => None,
    }
}

fn filter_stale_owner_dead_entity_packets(
    dead_entity_ids: &mut BTreeSet<u32>,
    packets: &mut Vec<ServerPacket>,
) {
    packets.retain(|packet| {
        if let Some(object_id) = owner_alive_entity_marker_object_id(packet) {
            dead_entity_ids.remove(&object_id);
        }
        if stale_dead_shared_entity_packet_object_id(packet)
            .is_some_and(|object_id| dead_entity_ids.contains(&object_id))
        {
            return false;
        }
        if let Some(object_id) = owner_dead_entity_marker_object_id(packet) {
            dead_entity_ids.insert(object_id);
        }
        true
    });
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

fn owner_zone_state_packets(
    owner_local_object_id: u32,
    packets: &[ServerPacket],
) -> Vec<ServerPacket> {
    packets
        .iter()
        .filter(|packet| match packet {
            ServerPacket::AddBuff { buff } => buff.object_id == owner_local_object_id,
            ServerPacket::RemoveBuff { object_id, .. }
            | ServerPacket::PauseBuff { object_id, .. }
            | ServerPacket::PlayerUpdate { object_id, .. }
            | ServerPacket::ObjectColourChanged { object_id, .. }
            | ServerPacket::ObjectGuildNameChanged { object_id, .. }
            | ServerPacket::ObjectName { object_id, .. }
            | ServerPacket::ObjectPoisoned { object_id, .. }
            | ServerPacket::ObjectLevelEffects { object_id, .. }
            | ServerPacket::MountUpdate { object_id, .. }
            | ServerPacket::FishingUpdate { object_id, .. }
            | ServerPacket::TransformUpdate { object_id, .. }
            | ServerPacket::ObjectHidden { object_id, .. }
            | ServerPacket::ObjectSneaking { object_id, .. }
            | ServerPacket::ObjectHide { object_id }
            | ServerPacket::ObjectShow { object_id } => *object_id == owner_local_object_id,
            ServerPacket::ObjectDied { info } => info.object_id == owner_local_object_id,
            ServerPacket::ObjectRevived { info } => info.object_id == owner_local_object_id,
            ServerPacket::ObjectEffect { info } => info.object_id == owner_local_object_id,
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
        let resources = self.resources_for_zone(zone_id);
        Box::new(SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(config),
            zone_state: resources.zone_state.clone(),
            account_inventory_service: self.account_inventory_service.clone(),
            npc_world_service: self.npc_world_service.clone(),
            economy_execution_context: None,
            last_ground_drop_projection_reconciliation_identity: None,
            trade_projection_reconciliation_state: TradeProjectionReconciliationState::Unknown,
            movement_ingress: SharedZoneMovementIngress::new(
                resources.movement_sender,
                resources.zone_state,
            ),
            shared_skill_item_request_seq: 0,
            force_next_zone_transform_sync: false,
            last_shared_entity_ids_by_map: BTreeMap::new(),
            last_shared_drop_ids_by_map: BTreeMap::new(),
            local_ground_drop_zone_ids: BTreeMap::new(),
            retired_local_ground_drop_ids: BTreeSet::new(),
            owner_dead_entity_ids: BTreeSet::new(),
            last_game_shop_purchase_outcome: None,
            #[cfg(test)]
            fail_next_npc_teleport_checkpoint_restore: false,
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

type SharedZoneMovementReply = Result<Option<WorldCommandExecution>, String>;

#[derive(Debug)]
struct SharedZoneMovementRequest {
    packet: ClientPacket,
    expected_presence_epoch: u64,
    session_state: Arc<Mutex<SharedZoneMovementSessionState>>,
    response_sender: SyncSender<SharedZoneMovementReply>,
}

#[derive(Debug, Default)]
struct SharedZoneMovementSessionState {
    presence_key: Option<ZonePresenceKey>,
    presence_epoch: u64,
    zone_move_seq: u64,
    pending_zone_player_movement: bool,
    recent_zone_player_movement_until_ms: u64,
    cached_map_file_name: Option<String>,
    cached_map_transfers: Vec<CachedMapTransfer>,
}

impl SharedZoneMovementSessionState {
    fn activate(
        &mut self,
        key: ZonePresenceKey,
        map_file_name: String,
        map_transfers: Vec<CachedMapTransfer>,
    ) {
        if self.presence_key.as_ref() != Some(&key)
            || self.cached_map_file_name.as_deref() != Some(map_file_name.as_str())
        {
            self.presence_epoch = self.presence_epoch.saturating_add(1);
        }
        self.presence_key = Some(key);
        self.cached_map_file_name = Some(map_file_name);
        self.cached_map_transfers = map_transfers;
    }

    fn deactivate(&mut self) -> Option<ZonePresenceKey> {
        let key = self.presence_key.take();
        if key.is_some() {
            self.presence_epoch = self.presence_epoch.saturating_add(1);
        }
        self.pending_zone_player_movement = false;
        self.recent_zone_player_movement_until_ms = 0;
        self.cached_map_file_name = None;
        self.cached_map_transfers.clear();
        key
    }

    fn note_player_movement_packets(&mut self, packets: &[ServerPacket], now_ms: u64) -> bool {
        let includes_user_location = packets_include_user_location(packets);
        self.pending_zone_player_movement = !includes_user_location;
        if includes_user_location {
            self.recent_zone_player_movement_until_ms =
                now_ms.saturating_add(SHARED_ZONE_POST_MOVEMENT_INPUT_GRACE_MS);
        }
        includes_user_location
    }

    fn ingress_safe_for(&self, map_file_name: &str, position: &Point) -> bool {
        let Some(cached_map_file_name) = self.cached_map_file_name.as_deref() else {
            return false;
        };
        let normalized_map = normalize_gateway_map_file_name(map_file_name);
        if normalize_gateway_map_file_name(cached_map_file_name) != normalized_map {
            return false;
        }

        !self.cached_map_transfers.iter().any(|transfer| {
            normalize_gateway_map_file_name(&transfer.map_file_name) == normalized_map
                && position.x
                    >= transfer
                        .min_x
                        .saturating_sub(SHARED_ZONE_TRANSFER_FALLBACK_MARGIN)
                && position.x
                    <= transfer
                        .max_x
                        .saturating_add(SHARED_ZONE_TRANSFER_FALLBACK_MARGIN)
                && position.y
                    >= transfer
                        .min_y
                        .saturating_sub(SHARED_ZONE_TRANSFER_FALLBACK_MARGIN)
                && position.y
                    <= transfer
                        .max_y
                        .saturating_add(SHARED_ZONE_TRANSFER_FALLBACK_MARGIN)
        })
    }
}

#[derive(Clone)]
pub(crate) struct SharedZoneMovementIngress {
    movement_sender: SyncSender<SharedZoneMovementRequest>,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    session_state: Arc<Mutex<SharedZoneMovementSessionState>>,
}

impl SharedZoneMovementIngress {
    fn new(
        movement_sender: SyncSender<SharedZoneMovementRequest>,
        zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    ) -> Self {
        Self {
            movement_sender,
            zone_state,
            session_state: Arc::new(Mutex::new(SharedZoneMovementSessionState::default())),
        }
    }

    pub(crate) fn register_live_outbound(
        &self,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<SharedZoneLiveOutboundRegistration>, String> {
        // Match movement execution's lock order: shared Zone first, then the
        // per-session presence epoch.
        let mut zone_state = self
            .zone_state
            .lock()
            .map_err(|_| "shared zone presence mutex is poisoned".to_string())?;
        let session_state = self
            .session_state
            .lock()
            .map_err(|_| "shared zone movement session mutex is poisoned".to_string())?;
        let Some(key) = session_state.presence_key.clone() else {
            return Ok(None);
        };
        let Some(session_id) = zone_state.zone_sessions.get(&key) else {
            return Ok(None);
        };
        if zone_state.zone_session_keys.get(session_id) != Some(&key)
            || !zone_state.players.contains_key(&key)
        {
            return Ok(None);
        }
        let registration_id = zone_state.register_live_zone_outbound(key.clone(), sender);
        drop(session_state);
        drop(zone_state);
        Ok(Some(SharedZoneLiveOutboundRegistration {
            zone_state: self.zone_state.clone(),
            key,
            registration_id,
        }))
    }

    pub(crate) fn try_execute(
        &self,
        packet: ClientPacket,
    ) -> Result<Option<WorldCommandExecution>, String> {
        if !matches!(
            &packet,
            ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. }
        ) {
            return Ok(None);
        }

        let expected_presence_epoch = {
            let session_state = self
                .session_state
                .lock()
                .map_err(|_| "shared zone movement session mutex is poisoned".to_string())?;
            if session_state.presence_key.is_none() {
                return Ok(None);
            }
            session_state.presence_epoch
        };
        let (response_sender, response_receiver) = sync_channel(1);
        let request = SharedZoneMovementRequest {
            packet,
            expected_presence_epoch,
            session_state: self.session_state.clone(),
            response_sender,
        };
        match self.movement_sender.try_send(request) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Ok(None),
            Err(TrySendError::Disconnected(_)) => {
                return Err("shared zone movement owner is unavailable".to_string());
            }
        }

        receive_shared_zone_movement_reply(&response_receiver, SHARED_ZONE_MOVEMENT_REPLY_TIMEOUT)
    }
}

fn receive_shared_zone_movement_reply(
    response_receiver: &Receiver<SharedZoneMovementReply>,
    timeout: Duration,
) -> SharedZoneMovementReply {
    match response_receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(format!(
            "shared zone movement owner timed out after {}ms",
            timeout.as_millis()
        )),
        Err(RecvTimeoutError::Disconnected) => {
            Err("shared zone movement owner dropped its response".to_string())
        }
    }
}

impl fmt::Debug for SharedZoneMovementIngress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("SharedZoneMovementIngress");
        match self.session_state.lock() {
            Ok(session_state) => debug
                .field("presence_key", &session_state.presence_key)
                .field("presence_epoch", &session_state.presence_epoch),
            Err(_) => debug.field("session_state", &"poisoned"),
        };
        debug.finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct SharedInProcessZoneResources {
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    movement_sender: SyncSender<SharedZoneMovementRequest>,
    tick_count: Arc<AtomicU64>,
    autonomous_ticks_enabled: Arc<AtomicBool>,
}

impl SharedInProcessZoneResources {
    fn new(
        zone_id: &ZoneId,
        cadence: Duration,
        mutation_capture: Arc<Mutex<Option<SharedZoneMutationCapture>>>,
        autonomous_ticks_enabled: bool,
    ) -> Self {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let tick_count = Arc::new(AtomicU64::new(0));
        let autonomous_ticks_enabled = Arc::new(AtomicBool::new(autonomous_ticks_enabled));
        let movement_sender = spawn_shared_zone_owner_with_cadence_and_counter(
            zone_id,
            zone_state.clone(),
            cadence,
            tick_count.clone(),
            mutation_capture,
            autonomous_ticks_enabled.clone(),
        );
        Self {
            zone_state,
            movement_sender,
            tick_count,
            autonomous_ticks_enabled,
        }
    }
}

impl fmt::Debug for SharedInProcessZoneResources {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedInProcessZoneResources")
            .field("zone_state", &"SharedInProcessZoneState")
            .field("movement_sender", &"SyncSender")
            .field("tick_count", &self.tick_count.load(Ordering::Relaxed))
            .field(
                "autonomous_ticks_enabled",
                &self.autonomous_ticks_enabled.load(Ordering::Relaxed),
            )
            .finish()
    }
}

#[derive(Debug)]
struct SharedZoneMovementExecution {
    packets: Vec<ServerPacket>,
    transform: Option<(Point, MirDirection)>,
    outcome: WorldCommandOutcome,
}

#[cfg(test)]
fn spawn_shared_zone_owner_with_cadence(
    zone_id: &ZoneId,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    cadence: Duration,
) -> SyncSender<SharedZoneMovementRequest> {
    spawn_shared_zone_owner_with_cadence_and_counter(
        zone_id,
        zone_state,
        cadence,
        Arc::new(AtomicU64::new(0)),
        Arc::new(Mutex::new(None)),
        Arc::new(AtomicBool::new(true)),
    )
}

fn spawn_shared_zone_owner_with_cadence_and_counter(
    zone_id: &ZoneId,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    cadence: Duration,
    tick_count: Arc<AtomicU64>,
    mutation_capture: Arc<Mutex<Option<SharedZoneMutationCapture>>>,
    autonomous_ticks_enabled: Arc<AtomicBool>,
) -> SyncSender<SharedZoneMovementRequest> {
    let (movement_sender, movement_receiver) = sync_channel(SHARED_ZONE_MOVEMENT_INGRESS_CAPACITY);
    let zone_id = zone_id.clone();
    thread::Builder::new()
        .name(format!("mir2-zone-owner-{}", zone_id.as_str()))
        .spawn(move || {
            shared_zone_owner_loop(
                zone_id,
                zone_state,
                movement_receiver,
                cadence,
                tick_count,
                mutation_capture,
                autonomous_ticks_enabled,
            )
        })
        .expect("shared zone owner thread should start");
    movement_sender
}

fn shared_zone_owner_loop(
    zone_id: ZoneId,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    movement_receiver: Receiver<SharedZoneMovementRequest>,
    cadence: Duration,
    tick_count: Arc<AtomicU64>,
    mutation_capture: Arc<Mutex<Option<SharedZoneMutationCapture>>>,
    autonomous_ticks_enabled: Arc<AtomicBool>,
) {
    let cadence = cadence.max(Duration::from_millis(1));
    let mut next_tick = Instant::now() + cadence;
    loop {
        let now = Instant::now();
        if now >= next_tick {
            if !autonomous_ticks_enabled.load(Ordering::Acquire) {
                next_tick = Instant::now() + cadence;
                continue;
            }
            let now_ms = shared_gateway_now_ms();
            let capture = mutation_capture
                .lock()
                .ok()
                .and_then(|capture| capture.clone());
            let result = if let Some(capture) = capture {
                let _gate = match capture.gate.lock_zone(&zone_id) {
                    Ok(gate) => gate,
                    Err(_) => {
                        eprintln!("shared zone cadence stopped: mutation gate poisoned");
                        return;
                    }
                };
                if (capture.authorize_tick)(&zone_id).is_err() {
                    // A still-running former primary is deliberately kept
                    // alive but frozen. A later Commonware generation may
                    // promote this host again, so do not terminate its owner
                    // loop on a fencing miss.
                    next_tick = Instant::now() + cadence;
                    continue;
                }
                run_shared_zone_cadence_tick(&zone_state, now_ms)
                    .and_then(|()| (capture.observer)(&zone_id, now_ms))
            } else {
                run_shared_zone_cadence_tick(&zone_state, now_ms)
            };
            if let Err(error) = result {
                eprintln!("shared zone cadence stopped: {error}");
                return;
            }
            tick_count.fetch_add(1, Ordering::Release);
            // Coalesce a late tick instead of replaying a burst of stale ticks.
            next_tick = Instant::now() + cadence;
            continue;
        }

        match movement_receiver.recv_timeout(next_tick.saturating_duration_since(now)) {
            Ok(request) => {
                let result = execute_shared_zone_movement(
                    &zone_state,
                    &request.session_state,
                    &request.packet,
                    Some(request.expected_presence_epoch),
                    true,
                    true,
                )
                .map(|execution| {
                    execution.map(|execution| WorldCommandExecution {
                        packets: execution.packets,
                        outcome: execution.outcome,
                        game_shop_purchase_outcome: None,
                    })
                });
                let _ = request.response_sender.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn run_shared_zone_cadence_tick(
    zone_state: &Arc<Mutex<SharedInProcessZoneState>>,
    now_ms: u64,
) -> Result<(), String> {
    let mut zone_state = zone_state
        .lock()
        .map_err(|_| "shared zone presence mutex is poisoned".to_string())?;
    // A teardown checkpoint must observe a quiescent Zone. ZoneManager only
    // exposes a global Tick, so pause autonomous mutations while any player is
    // frozen; explicit teardown performs one final fenced drain itself.
    if zone_state.any_teardown_fenced() {
        return Ok(());
    }
    #[cfg(test)]
    {
        zone_state.zone_cadence_tick_count = zone_state.zone_cadence_tick_count.saturating_add(1);
    }
    let outbounds = zone_state.zone_manager.handle(ZoneCommand::Tick { now_ms });
    let _ = zone_state.dispatch_zone_outbounds(outbounds, None);

    let map_file_names = zone_state.maps.keys().cloned().collect::<Vec<_>>();
    for map_file_name in map_file_names {
        let _ = zone_state.expire_shared_drops(&map_file_name, None, now_ms);
    }
    Ok(())
}

fn execute_shared_zone_movement(
    zone_state: &Arc<Mutex<SharedInProcessZoneState>>,
    session_state: &Arc<Mutex<SharedZoneMovementSessionState>>,
    packet: &ClientPacket,
    expected_presence_epoch: Option<u64>,
    require_ingress_safe: bool,
    defer_owner_transform: bool,
) -> Result<Option<SharedZoneMovementExecution>, String> {
    if !matches!(
        packet,
        ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. }
    ) {
        return Ok(None);
    }

    let mut zone_state = zone_state
        .lock()
        .map_err(|_| "shared zone presence mutex is poisoned".to_string())?;
    let mut session_state = session_state
        .lock()
        .map_err(|_| "shared zone movement session mutex is poisoned".to_string())?;
    if expected_presence_epoch.is_some_and(|expected| expected != session_state.presence_epoch) {
        return Ok(None);
    }
    let Some(key) = session_state.presence_key.clone() else {
        return Ok(None);
    };
    if zone_state.teardown_fenced(&key) {
        return Ok(None);
    }
    let Some(presence) = zone_state.players.get(&key) else {
        return Ok(None);
    };
    let map_file_name = presence.map_file_name.clone();
    let active_identity = ActiveSessionIdentity {
        account_id: key.account_id.clone(),
        character_index: key.character_index,
        character_name: presence.entity.name.clone(),
    };
    let position = Point {
        x: presence.entity.x,
        y: presence.entity.y,
    };
    if require_ingress_safe && !session_state.ingress_safe_for(&map_file_name, &position) {
        return Ok(None);
    }
    let Some(session_id) = zone_state.zone_sessions.get(&key).cloned() else {
        return Ok(None);
    };
    if zone_state.zone_session_keys.get(&session_id) != Some(&key) {
        return Ok(None);
    }

    let now_ms = shared_gateway_now_ms();
    let movement_command = match packet {
        ClientPacket::Walk { direction } => {
            session_state.zone_move_seq = session_state.zone_move_seq.saturating_add(1);
            ZoneCommand::Walk {
                session_id: session_id.clone(),
                direction: *direction,
                seq: session_state.zone_move_seq,
                now_ms,
            }
        }
        ClientPacket::Run { direction } => {
            session_state.zone_move_seq = session_state.zone_move_seq.saturating_add(1);
            ZoneCommand::Run {
                session_id: session_id.clone(),
                direction: *direction,
                seq: session_state.zone_move_seq,
                now_ms,
            }
        }
        ClientPacket::Turn { direction } => ZoneCommand::Turn {
            session_id: session_id.clone(),
            direction: *direction,
            now_ms,
        },
        _ => return Ok(None),
    };
    let mut outbounds = zone_state.zone_manager.handle(movement_command);
    outbounds.extend(
        zone_state
            .zone_manager
            .handle(ZoneCommand::TickPlayerMovement { session_id, now_ms }),
    );
    let (packets, transform, _, _, _, _, _) =
        zone_state.dispatch_zone_outbounds(outbounds, Some(&key));
    if defer_owner_transform {
        if let Some((position, direction)) = transform.as_ref() {
            zone_state
                .pending_zone_transforms
                .insert(key, (position.clone(), *direction));
        }
    }
    session_state.note_player_movement_packets(&packets, now_ms);

    let outcome = WorldCommandOutcome {
        command_kind: WorldCommand::ClientPacket(packet.clone()).kind(),
        packet_count: packets.len(),
        snapshot_tick: 0,
        active_identity: Some(active_identity),
    };

    Ok(Some(SharedZoneMovementExecution {
        packets,
        transform,
        outcome,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TradeProjectionReconciliationState {
    Unknown,
    Clear(ActiveSessionIdentity),
    Pending(ActiveSessionIdentity),
}

struct SharedInProcessZoneSessionRuntime {
    inner: InProcessWorldRuntime,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    account_inventory_service: SharedAccountInventoryServiceHandle,
    npc_world_service: SharedNpcWorldServiceHandle,
    economy_execution_context: Option<SharedAccountInventoryExecutionContext>,
    last_ground_drop_projection_reconciliation_identity: Option<ActiveSessionIdentity>,
    trade_projection_reconciliation_state: TradeProjectionReconciliationState,
    movement_ingress: SharedZoneMovementIngress,
    shared_skill_item_request_seq: u64,
    force_next_zone_transform_sync: bool,
    last_shared_entity_ids_by_map: BTreeMap<String, BTreeSet<u32>>,
    last_shared_drop_ids_by_map: BTreeMap<String, BTreeSet<u32>>,
    local_ground_drop_zone_ids: BTreeMap<(String, u32), u32>,
    retired_local_ground_drop_ids: BTreeSet<(String, u32)>,
    owner_dead_entity_ids: BTreeSet<u32>,
    last_game_shop_purchase_outcome: Option<GameShopPurchaseOutcome>,
    #[cfg(test)]
    fail_next_npc_teleport_checkpoint_restore: bool,
}

pub(crate) fn shared_zone_movement_ingress(
    runtime: &ZoneRuntimeHandle,
) -> Option<SharedZoneMovementIngress> {
    runtime
        .as_ref()
        .as_any()
        .downcast_ref::<SharedInProcessZoneSessionRuntime>()
        .map(|runtime| runtime.movement_ingress.clone())
}

pub(crate) fn sync_zone_movement_transform(runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
    let Some(runtime) = runtime
        .as_mut()
        .as_any_mut()
        .downcast_mut::<SharedInProcessZoneSessionRuntime>()
    else {
        return Ok(());
    };
    runtime.sync_pending_zone_movement_transform()
}

pub(crate) fn prepare_zone_teardown_checkpoint(
    runtime: &mut ZoneRuntimeHandle,
    owner_lease: &ZoneOwnerLease,
) -> Result<Option<PreparedZoneTeardown>, String> {
    if let Some(shared) = runtime
        .as_mut()
        .as_any_mut()
        .downcast_mut::<SharedInProcessZoneSessionRuntime>()
    {
        return shared.prepare_teardown_checkpoint(owner_lease);
    }
    let Some(identity) = runtime.active_identity() else {
        return Ok(None);
    };
    let checkpoint = runtime
        .active_character_checkpoint()
        .ok_or_else(|| "teardown identity has no active character checkpoint".to_string())?;
    let prepared = PreparedZoneTeardown::new(owner_lease.clone(), identity, checkpoint);
    prepared.validate_identity_checkpoint()?;
    Ok(Some(prepared))
}

pub(crate) fn persist_zone_teardown_checkpoint(
    runtime: &mut ZoneRuntimeHandle,
    prepared: &PreparedZoneTeardown,
) -> Result<(), String> {
    prepared.validate_identity_checkpoint()?;
    let identity = runtime
        .active_identity()
        .ok_or_else(|| "teardown persist requires the prepared active identity".to_string())?;
    if identity != prepared.identity {
        return Err("teardown persist active identity changed after preparation".to_string());
    }
    runtime.restore_active_character_checkpoint(prepared.checkpoint())?;
    runtime.save_active_character()
}

pub(crate) fn release_zone_teardown_fence(runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
    let Some(shared) = runtime
        .as_mut()
        .as_any_mut()
        .downcast_mut::<SharedInProcessZoneSessionRuntime>()
    else {
        return Ok(());
    };
    shared.release_teardown_fence()
}

#[cfg(test)]
pub(crate) fn zone_teardown_is_fenced(runtime: &ZoneRuntimeHandle) -> bool {
    runtime
        .as_ref()
        .as_any()
        .downcast_ref::<SharedInProcessZoneSessionRuntime>()
        .is_some_and(SharedInProcessZoneSessionRuntime::teardown_is_fenced)
}

impl fmt::Debug for SharedInProcessZoneSessionRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedInProcessZoneSessionRuntime")
            .field("inner", &"InProcessWorldRuntime")
            .field("movement_ingress", &self.movement_ingress)
            .finish()
    }
}

impl SharedInProcessZoneSessionRuntime {
    fn zone_now_ms() -> u64 {
        shared_gateway_now_ms()
    }

    fn set_economy_execution_context(
        &mut self,
        context: Option<SharedAccountInventoryExecutionContext>,
    ) {
        self.economy_execution_context = context;
    }

    fn next_in_process_economy_execution_context(
        &self,
        owner_lease: &ZoneOwnerLease,
    ) -> Option<SharedAccountInventoryExecutionContext> {
        let source_sequence = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .next_economy_source_sequence()?;
        Some(SharedAccountInventoryExecutionContext {
            zone_id: owner_lease.zone_id().clone(),
            fencing_generation: owner_lease.fencing_token(),
            source_sequence,
            created_at_ms: shared_gateway_now_ms(),
            external_commit_authorized: true,
        })
    }

    fn rebind_account_store(&mut self, authoritative: &GatewayConfig) {
        self.inner.rebind_account_store(authoritative);
    }

    fn commit_account_inventory_outcome(
        &mut self,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        let is_ground_drop_projection = matches!(
            &envelope.command,
            SharedAccountInventoryCommand::GroundDropPickup(_)
                | SharedAccountInventoryCommand::GroundDropClaimPickup { .. }
        );
        let service = Arc::clone(&self.account_inventory_service);
        let context = self.economy_execution_context.clone();
        let outcome = service.commit_fenced(&mut self.inner, context.as_ref(), envelope);
        if is_ground_drop_projection
            && matches!(
                &outcome,
                SharedAccountInventoryCommitOutcome::Confirmed(receipt) if receipt.committed
            )
        {
            // The durable row may already be projected, but invalidating this
            // cache costs one bounded query and guarantees that an immediate
            // save/capacity failure is retried for the owning character.
            self.last_ground_drop_projection_reconciliation_identity = None;
        }
        outcome
    }

    fn retry_account_inventory_outcome(
        &mut self,
        recovery_context: &SharedAccountInventoryExecutionContext,
        expected_idempotency_key: &str,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        let service = Arc::clone(&self.account_inventory_service);
        let outcome = service.retry_commit_fenced(
            &mut self.inner,
            Some(recovery_context),
            expected_idempotency_key,
            envelope,
        );
        if matches!(
            &outcome,
            SharedAccountInventoryCommitOutcome::Confirmed(receipt) if receipt.committed
        ) {
            self.last_ground_drop_projection_reconciliation_identity = None;
        }
        outcome
    }

    fn commit_account_inventory(
        &mut self,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.commit_account_inventory_outcome(envelope)
            .into_receipt()
    }

    fn bootstrap_account_inventory(&self) -> bool {
        self.account_inventory_service
            .bootstrap_fenced(&self.inner, self.economy_execution_context.as_ref())
    }

    fn settle_shared_trade(
        &self,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        self.account_inventory_service.settle_trade_fenced(
            self.economy_execution_context.as_ref(),
            first,
            second,
        )
    }

    fn retry_shared_trade(
        &self,
        recovery_context: &SharedAccountInventoryExecutionContext,
        expected_idempotency_key: &str,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        self.account_inventory_service.retry_trade_fenced(
            Some(recovery_context),
            expected_idempotency_key,
            first,
            second,
        )
    }

    fn recent_zone_player_movement_input_window_active(&self, now_ms: u64) -> bool {
        self.movement_ingress
            .session_state
            .lock()
            .expect("shared zone movement session mutex should not be poisoned")
            .recent_zone_player_movement_until_ms
            > now_ms
    }

    fn cancel_pending_zone_player_movement(&mut self) -> Vec<ServerPacket> {
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        let now_ms = Self::zone_now_ms();
        let packets = self
            .dispatch_zone_player_command(ZoneCommand::CancelPendingMovement { session_id }, false);
        let mut movement = self
            .movement_ingress
            .session_state
            .lock()
            .expect("shared zone movement session mutex should not be poisoned");
        movement.pending_zone_player_movement = false;
        if packets_include_user_location(&packets) {
            movement.recent_zone_player_movement_until_ms =
                now_ms.saturating_add(SHARED_ZONE_POST_MOVEMENT_INPUT_GRACE_MS);
        }
        packets
    }

    fn refresh_replica_zone_binding(&mut self) -> Result<(), String> {
        let Some(identity) = self.inner.active_identity() else {
            return Ok(());
        };
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name else {
            return Ok(());
        };
        let cached_map_transfers = snapshot
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
            .collect::<Vec<_>>();
        let key = ZonePresenceKey::from_identity(&identity);
        let last_seen_move_seq = {
            let zone_state = self
                .zone_state
                .lock()
                .map_err(|_| "shared zone presence mutex is poisoned".to_string())?;
            let Some(session_id) = zone_state.zone_sessions.get(&key) else {
                return Ok(());
            };
            zone_state
                .zone_manager
                .player_last_seen_move_seq(session_id)
                .unwrap_or_default()
        };
        let mut movement = self
            .movement_ingress
            .session_state
            .lock()
            .map_err(|_| "shared zone movement session mutex is poisoned".to_string())?;
        movement.activate(key, map_file_name, cached_map_transfers);
        movement.zone_move_seq = last_seen_move_seq;
        Ok(())
    }

    fn next_shared_economy_request_id(&mut self) -> u64 {
        if let Some(context) = self.economy_execution_context.as_ref() {
            return context.source_sequence;
        }
        self.shared_skill_item_request_seq = self.shared_skill_item_request_seq.saturating_add(1);
        self.shared_skill_item_request_seq
    }

    fn execute_shared_gold_drop(&mut self, amount: u32) -> Vec<ServerPacket> {
        let Some(identity) = self.inner.active_identity() else {
            return Vec::new();
        };
        let request_id = self.next_shared_economy_request_id();
        let receipt = self.commit_account_inventory(SharedAccountInventoryCommandEnvelope {
            identity,
            command: SharedAccountInventoryCommand::GoldDrop { amount, request_id },
        });
        debug_assert_eq!(
            receipt.kind,
            SharedAccountInventoryTransactionKind::GoldDrop
        );
        receipt.packets
    }

    fn execute_shared_inventory_item_drop(
        &mut self,
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    ) -> Option<Vec<ServerPacket>> {
        let drop = self
            .inner
            .shared_inventory_item_drop(unique_id, count, hero_inventory)?;
        let identity = self.inner.active_identity()?;
        let request_id = self.next_shared_economy_request_id();
        let receipt = self.commit_account_inventory(SharedAccountInventoryCommandEnvelope {
            identity,
            command: SharedAccountInventoryCommand::InventoryItemDrop { drop, request_id },
        });
        debug_assert_eq!(
            receipt.kind,
            SharedAccountInventoryTransactionKind::InventoryItemDrop
        );
        Some(receipt.packets)
    }

    fn filter_stale_owner_dead_entity_packets(&mut self, packets: &mut Vec<ServerPacket>) {
        filter_stale_owner_dead_entity_packets(&mut self.owner_dead_entity_ids, packets);
    }

    fn apply_zone_transform(&mut self, transform: Option<(Point, MirDirection)>) {
        if let Some((position, direction)) = transform {
            self.inner
                .force_authoritative_player_transform(position, direction);
        }
    }

    fn sync_pending_zone_movement_transform(&mut self) -> Result<(), String> {
        let key = self
            .movement_ingress
            .session_state
            .lock()
            .map_err(|_| "shared zone movement session mutex is poisoned".to_string())?
            .presence_key
            .clone();
        let Some(key) = key else {
            return Ok(());
        };
        let transform = self
            .zone_state
            .lock()
            .map_err(|_| "shared zone presence mutex is poisoned".to_string())?
            .take_pending_zone_transform(&key);
        self.apply_zone_transform(transform);
        // A presence can already be authoritative even when its one-shot pending
        // transform was consumed by an earlier packet. Persist from the current
        // shared-Zone position, never from a stale private-runtime coordinate.
        self.force_inner_to_current_zone_transform();
        Ok(())
    }

    fn force_inner_to_current_zone_transform(&mut self) {
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = self.authoritative_self_entity_for_snapshot(&snapshot) else {
            return;
        };
        self.inner.force_authoritative_player_transform(
            Point {
                x: self_entity.x,
                y: self_entity.y,
            },
            self_entity.direction,
        );
    }

    fn force_inner_to_current_zone_vitals(&mut self) {
        let Some(key) = self.current_presence_key() else {
            return;
        };
        let vitals = {
            let zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            zone_state
                .zone_sessions
                .get(&key)
                .and_then(|session_id| zone_state.zone_manager.player_vitals(session_id))
        };
        let Some((hp, _max_hp, mp)) = vitals else {
            return;
        };
        let snapshot = self.inner.world_snapshot();
        // A private-session monster/hazard tick can deliver the Crystal self
        // death packet before that transition is reflected in the shared Zone.
        // The Zone's pre-death HP (often the native-combat 1 HP floor) must not
        // silently revive the private runtime on every following world tick:
        // that used to emit Death/ObjectDied repeatedly and made TownRevive a
        // no-op because the packet handler observed a living player. Preserve
        // the acknowledged private death until an explicit revive emits
        // ObjectRevived and synchronizes both authorities below.
        let local_player_is_acknowledged_dead = snapshot.player_hp == Some(0)
            && snapshot
                .player_object_id
                .is_some_and(|object_id| self.owner_dead_entity_ids.contains(&object_id));
        if local_player_is_acknowledged_dead && hp > 0 {
            return;
        }
        if snapshot.player_hp != Some(hp) || snapshot.player_mp != Some(mp) {
            self.inner
                .force_authoritative_player_vitals(Some(hp), Some(mp));
        }
    }

    fn apply_zone_shout_consume(&mut self, consume: Option<(bool, bool)>) {
        if let Some((map_shout, server_shout)) = consume {
            self.inner
                .consume_zone_chat_shout_permission(map_shout, server_shout);
        }
    }

    fn current_zone_transfer_key(&self) -> Option<String> {
        let (normalized_map, key, cached_map_transfers) = {
            let session_state = self
                .movement_ingress
                .session_state
                .lock()
                .expect("shared zone movement session mutex should not be poisoned");
            (
                normalize_gateway_map_file_name(session_state.cached_map_file_name.as_deref()?),
                session_state.presence_key.clone()?,
                session_state.cached_map_transfers.clone(),
            )
        };
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

        cached_map_transfers
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
        let cached_map_transfers = snapshot
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
            .collect::<Vec<_>>();

        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .cloned();
        let mut shared_entities = self.inner.current_map_shared_entity_snapshots();
        // Personal world snapshots intentionally contain only the player's
        // current Crystal data range. Seed every configured NPC and native
        // monster on the active map into the shared Zone up front so
        // low-latency movement can reveal distant world objects through AOI
        // without requiring an unrelated personal-session command first.
        shared_entities.retain(|entity| !self.owner_dead_entity_ids.contains(&entity.object_id));
        shared_entities.sort_by_key(|entity| entity.object_id);
        shared_entities.dedup_by_key(|entity| entity.object_id);
        let mut native_monster_spawns = shared_entities
            .iter()
            .filter(|entity| entity.kind == WorldEntityKind::Monster && !entity.dead)
            .filter_map(|entity| self.inner.zone_monster_spawn_snapshot(entity.object_id))
            .collect::<Vec<_>>();
        native_monster_spawns.sort_by_key(|spawn| spawn.object_id);
        native_monster_spawns.dedup_by_key(|spawn| spawn.object_id);
        let shared_npc_spawn_packets = shared_entities
            .iter()
            .filter(|entity| entity.kind == WorldEntityKind::Npc)
            .filter_map(shared_entity_spawn_packet)
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
        let mut shared_ground_drops =
            self.current_snapshot_ground_drops_for_shared_state(&snapshot, &zone_state, Some(&key));
        let shared_drop_ids = shared_ground_drops
            .iter()
            .map(|drop| drop.object_id)
            .collect::<BTreeSet<_>>();
        let previous_drop_ids = self
            .last_shared_drop_ids_by_map
            .insert(map_file_name.clone(), shared_drop_ids)
            .unwrap_or_default();
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
        let zone_object_id = zone_state.upsert_player_with_transform_policy(
            key.clone(),
            &identity.character_name,
            map_file_name.clone(),
            self_entity,
            snapshot.free_bag_slots,
            snapshot.player_pk_points,
            !allow_transform_sync,
        );
        let local_self_object_id = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| entity.object_id);
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
                if !shared_npc_spawn_packets.is_empty() {
                    outbounds.extend(zone_state.zone_manager.handle(
                        ZoneCommand::SyncSharedObjects {
                            session_id: session_id.clone(),
                            packets: shared_npc_spawn_packets.clone(),
                            // StartGame/map-transfer already returns the owner's
                            // initial viewport. Seed the shared Zone for later
                            // movement and other observers without duplicating
                            // those packets back to this client.
                            include_owner: false,
                            now_ms,
                        },
                    ));
                }
                if !native_monster_spawns.is_empty() {
                    outbounds.extend(zone_state.zone_manager.handle(
                        ZoneCommand::SyncNativeMonsters {
                            session_id: session_id.clone(),
                            monsters: native_monster_spawns.clone(),
                            now_ms,
                        },
                    ));
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
                    .handle(ZoneCommand::SyncPlayerVitals {
                        session_id: session_id.clone(),
                        hp: join.hp,
                        max_hp: join.max_hp,
                        mp: join.mp,
                    }),
            );
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
            if !shared_npc_spawn_packets.is_empty() {
                outbounds.extend(
                    zone_state
                        .zone_manager
                        .handle(ZoneCommand::SyncSharedObjects {
                            session_id: session_id.clone(),
                            packets: shared_npc_spawn_packets,
                            include_owner: true,
                            now_ms,
                        }),
                );
            }
            if !native_monster_spawns.is_empty() {
                outbounds.extend(
                    zone_state
                        .zone_manager
                        .handle(ZoneCommand::SyncNativeMonsters {
                            session_id: session_id.clone(),
                            monsters: native_monster_spawns,
                            now_ms,
                        }),
                );
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
        drop(zone_state);
        self.movement_ingress
            .session_state
            .lock()
            .expect("shared zone movement session mutex should not be poisoned")
            .activate(key, map_file_name, cached_map_transfers);
        self.apply_zone_player_buff_packets(&packets);
        self.inner.apply_shared_monster_lifecycle_packets(&packets);
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        packets
    }

    fn remove_presence(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self
            .movement_ingress
            .session_state
            .lock()
            .expect("shared zone movement session mutex should not be poisoned")
            .deactivate()
        else {
            return Vec::new();
        };
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        let outbounds = zone_state.remove_player(&key);
        let (mut packets, transform, shout_consume, _, _, player_damages, player_heals) =
            zone_state.dispatch_zone_outbounds(outbounds, Some(&key));
        zone_state.forget_zone_session(&key);
        drop(zone_state);
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        packets
    }

    fn current_presence_key(&self) -> Option<ZonePresenceKey> {
        self.movement_ingress
            .session_state
            .lock()
            .expect("shared zone movement session mutex should not be poisoned")
            .presence_key
            .clone()
            .or_else(|| {
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

    fn sync_newly_active_private_monsters_to_zone(&mut self) -> Vec<ServerPacket> {
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        self.inner.reconcile_current_map_monster_activation();
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return Vec::new();
        };
        let active_monsters = self
            .inner
            .current_map_shared_entity_snapshots()
            .into_iter()
            .filter(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && !entity.dead
                    && !entity.hp.is_some_and(|hp| hp <= 0)
                    && !self.owner_dead_entity_ids.contains(&entity.object_id)
            })
            .collect::<Vec<_>>();
        let zone_key = ZoneKey::for_map(map_file_name);
        let missing_object_ids = {
            let zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            active_monsters
                .iter()
                .filter(|entity| {
                    !zone_state
                        .zone_manager
                        .zone(&zone_key)
                        .is_some_and(|zone| zone.retains_object_id(entity.object_id))
                })
                .map(|entity| entity.object_id)
                .collect::<Vec<_>>()
        };
        let monsters = missing_object_ids
            .into_iter()
            .filter_map(|object_id| self.inner.zone_monster_spawn_snapshot(object_id))
            .collect::<Vec<_>>();
        if monsters.is_empty() {
            return Vec::new();
        }
        self.dispatch_zone_player_command(
            ZoneCommand::SyncNativeMonsters {
                session_id,
                monsters,
                now_ms: Self::zone_now_ms(),
            },
            false,
        )
    }

    fn authoritative_self_entity_for_snapshot(
        &self,
        snapshot: &WorldSnapshot,
    ) -> Option<WorldEntitySnapshot> {
        let mut self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .cloned()?;
        let (Some(key), Some(map_file_name)) = (
            self.current_presence_key(),
            snapshot.map_file_name.as_deref(),
        ) else {
            return Some(self_entity);
        };
        let zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        if let Some(presence) = zone_state.players.get(&key) {
            if presence.map_file_name == map_file_name {
                self_entity.x = presence.entity.x;
                self_entity.y = presence.entity.y;
                self_entity.direction = presence.entity.direction;
            }
        }
        Some(self_entity)
    }

    fn authoritative_zone_owner_correction(&self) -> Vec<ServerPacket> {
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = self.authoritative_self_entity_for_snapshot(&snapshot) else {
            return Vec::new();
        };
        vec![ServerPacket::UserLocation {
            location: UserLocation {
                position: Point {
                    x: self_entity.x,
                    y: self_entity.y,
                },
                direction: self_entity.direction,
            },
        }]
    }

    /// Refresh one attack admission from the authenticated personal session at
    /// the command boundary. This internal command has no BrowserCommand or
    /// raw ClientPacket representation.
    fn sync_authoritative_zone_combat_state(
        &mut self,
        session_id: &SessionId,
    ) -> Option<Vec<ServerPacket>> {
        // `inner` is the authenticated personal SimulationSession runtime. Its
        // WorldSnapshot is rebuilt from current ECS/resources on every call;
        // optional predicates stay fail-closed rather than becoming `false`.
        let snapshot = self.inner.world_snapshot();
        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)?;
        let class = self_entity.class?;
        let has_class_weapon = self_entity.has_class_weapon?;
        let riding_mount = self_entity.riding_mount?;
        let mount_attack_allowed = self_entity.can_mount_attack?;
        let fishing = self_entity.fishing?;
        let dead = self_entity.dead || snapshot.player_hp? <= 0;
        // Mirrors Simulation combat's complete attack-blocking set. These are
        // authoritative active player buffs, not client-provided snapshot data.
        let attack_blocked = snapshot.active_buffs.iter().any(|buff| {
            matches!(
                buff.key.as_str(),
                "crystal-paralysis"
                    | "crystal-dazed"
                    | "crystal-stun"
                    | "crystal-frozen"
                    | "crystal-blindness"
            )
        });
        Some(self.dispatch_zone_player_command(
            ZoneCommand::sync_player_combat_state(
                session_id.clone(),
                class,
                has_class_weapon,
                riding_mount,
                mount_attack_allowed,
                dead,
                attack_blocked,
                fishing,
            ),
            false,
        ))
    }

    fn sync_current_shared_ground_drops_to_zone(&mut self, session_id: &SessionId) {
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.clone();
        let current_key = self.current_presence_key();
        let drops = {
            let zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let mut drops = zone_state
                .map_layer(map_file_name.as_deref())
                .map(|layer| layer.ground_drops)
                .unwrap_or_default();
            for drop in self.current_snapshot_ground_drops_for_shared_state(
                &snapshot,
                &zone_state,
                current_key.as_ref(),
            ) {
                drops.entry(drop.object_id).or_insert(drop);
            }
            drops.into_values().collect::<Vec<_>>()
        };
        if drops.is_empty() {
            return;
        }
        let _ = self.dispatch_zone_player_command(
            ZoneCommand::SyncGroundDrops {
                session_id: session_id.clone(),
                drops,
                now_ms: Self::zone_now_ms(),
            },
            false,
        );
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
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&packets);
        self.inner.apply_shared_monster_lifecycle_packets(&packets);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets_by_object_id, canceled_claims) =
            self.apply_zone_ground_drop_claims(ground_drop_claims);
        merge_ground_drop_claim_packets_in_crystal_order(
            &mut packets,
            claim_packets_by_object_id,
            &canceled_claims,
        );
        packets
    }

    fn prepare_teardown_checkpoint(
        &mut self,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        let Some(identity) = self.inner.active_identity() else {
            return Ok(None);
        };
        let key = ZonePresenceKey::from_identity(&identity);
        let (
            mut packets,
            pending_transform,
            shout_consume,
            ground_drop_claims,
            monster_kill_awards,
            player_damages,
            player_heals,
            authoritative_transform,
            authoritative_vitals,
        ) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .map_err(|_| "shared zone presence mutex is poisoned".to_string())?;
            zone_state.begin_teardown_fence(&key)?;
            let session_id = zone_state
                .zone_sessions
                .get(&key)
                .cloned()
                .ok_or_else(|| "fenced shared Zone presence has no session id".to_string())?;
            let pending = (
                zone_state.take_pending_zone_packets(&key),
                zone_state.take_pending_zone_transform(&key),
                zone_state.take_pending_zone_shout_consume(&key),
                zone_state.take_pending_zone_ground_drop_claims(&key),
                zone_state.take_pending_zone_monster_kill_awards(&key),
                zone_state.take_pending_zone_player_damages(&key),
                zone_state.take_pending_zone_player_heals(&key),
            );
            (
                pending.0,
                pending.1,
                pending.2,
                pending.3,
                pending.4,
                pending.5,
                pending.6,
                zone_state.zone_manager.player_transform(&session_id),
                zone_state.zone_manager.player_vitals(&session_id),
            )
        };

        let authoritative_transform = authoritative_transform.ok_or_else(|| {
            "fenced shared Zone presence has no authoritative transform".to_string()
        })?;
        let authoritative_vitals = authoritative_vitals
            .ok_or_else(|| "fenced shared Zone presence has no authoritative vitals".to_string())?;

        // Deterministic drain order is part of the persistence contract. Packet
        // lifecycle effects are applied before rewards and claims, and the final
        // Zone snapshot wins over any stale private mirror.
        self.apply_zone_transform(pending_transform);
        self.apply_zone_shout_consume(shout_consume);
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&packets);
        self.inner.apply_shared_monster_lifecycle_packets(&packets);
        self.apply_zone_monster_kill_awards_checked(&key, monster_kill_awards)?;
        let (claim_packets_by_object_id, canceled_claims) =
            self.apply_zone_ground_drop_claims(ground_drop_claims);
        merge_ground_drop_claim_packets_in_crystal_order(
            &mut packets,
            claim_packets_by_object_id,
            &canceled_claims,
        );

        // A transport teardown has no ordered/fenced economy command context.
        // Apply only decisions that were already finalized before teardown;
        // unresolved PostgreSQL outcomes remain in the recovery ledger for the
        // next authenticated command. Retrying them here could turn a missing
        // commit acknowledgement into a false rejection and duplicate assets.
        packets.extend(self.apply_finalized_shared_trade_packets());
        packets.extend(
            self.cancel_pending_shared_trade_offers_for_character(Some(&identity.character_name)),
        );

        self.inner.force_authoritative_player_transform(
            authoritative_transform.0,
            authoritative_transform.1,
        );
        self.inner.force_authoritative_player_vitals(
            Some(authoritative_vitals.0),
            Some(authoritative_vitals.2),
        );
        let checkpoint = self
            .inner
            .active_character_checkpoint()
            .ok_or_else(|| "teardown drain produced no active checkpoint".to_string())?;
        let prepared = PreparedZoneTeardown::new(owner_lease.clone(), identity, checkpoint);
        prepared.validate_identity_checkpoint()?;
        Ok(Some(prepared))
    }

    fn release_teardown_fence(&mut self) -> Result<(), String> {
        let Some(key) = self.current_presence_key() else {
            return Ok(());
        };
        self.zone_state
            .lock()
            .map_err(|_| "shared zone presence mutex is poisoned".to_string())?
            .release_teardown_fence(&key);
        Ok(())
    }

    fn teardown_is_fenced(&self) -> bool {
        self.current_presence_key().is_some_and(|key| {
            self.zone_state
                .lock()
                .map(|state| state.teardown_fenced(&key))
                .unwrap_or(true)
        })
    }

    fn dispatch_zone_player_command(
        &mut self,
        command: ZoneCommand,
        tick_after_command: bool,
    ) -> Vec<ServerPacket> {
        let (mut packets, ground_drop_claims, monster_kill_awards, player_damages, player_heals) =
            self.dispatch_zone_player_command_collecting_claims(command, tick_after_command);
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&packets);
        self.inner.apply_shared_monster_lifecycle_packets(&packets);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets_by_object_id, canceled_claims) =
            self.apply_zone_ground_drop_claims(ground_drop_claims);
        merge_ground_drop_claim_packets_in_crystal_order(
            &mut packets,
            claim_packets_by_object_id,
            &canceled_claims,
        );
        packets
    }

    fn dispatch_zone_player_command_collecting_claims(
        &mut self,
        command: ZoneCommand,
        tick_after_command: bool,
    ) -> (
        Vec<ServerPacket>,
        Vec<GroundDropClaimTicket>,
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
        Vec<GroundDropClaimTicket>,
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
                if zone_state.command_mutates_teardown_fence(&command) {
                    continue;
                }
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

    fn apply_zone_player_damages(&mut self, damages: Vec<i32>) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        for damage in damages {
            if self.inner.apply_zone_player_damage(damage) {
                packets.extend(self.inner.apply_zone_player_death_penalty());
            }
        }
        packets
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

    fn apply_zone_monster_kill_awards_checked(
        &mut self,
        key: &ZonePresenceKey,
        awards: Vec<ZoneMonsterKillAward>,
    ) -> Result<Vec<ServerPacket>, String> {
        if awards.is_empty() {
            return Ok(Vec::new());
        }
        let Some(identity) = self.inner.active_identity() else {
            self.zone_state
                .lock()
                .map_err(|_| "shared zone presence mutex is poisoned".to_string())?
                .prepend_zone_monster_kill_awards(key.clone(), awards);
            return Err("monster kill award drain requires an active identity".to_string());
        };

        let mut packets = Vec::new();
        let mut awards = awards.into_iter();
        while let Some(award) = awards.next() {
            let retry_award = award.clone();
            let receipt = self.commit_account_inventory(SharedAccountInventoryCommandEnvelope {
                identity: identity.clone(),
                command: SharedAccountInventoryCommand::MonsterKillAward(award),
            });
            debug_assert_eq!(
                receipt.kind,
                SharedAccountInventoryTransactionKind::MonsterKillAward
            );
            if !receipt.committed {
                let mut retry = vec![retry_award];
                retry.extend(awards);
                self.zone_state
                    .lock()
                    .map_err(|_| "shared zone presence mutex is poisoned".to_string())?
                    .prepend_zone_monster_kill_awards(key.clone(), retry);
                return Err(
                    "monster kill award economy commit failed during teardown drain".to_string(),
                );
            }
            packets.extend(receipt.packets);
        }
        Ok(packets)
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
                let receipt =
                    self.commit_account_inventory(SharedAccountInventoryCommandEnvelope {
                        identity: identity.clone(),
                        command: SharedAccountInventoryCommand::MonsterKillAward(award),
                    });
                debug_assert_eq!(
                    receipt.kind,
                    SharedAccountInventoryTransactionKind::MonsterKillAward
                );
                receipt.packets
            })
            .collect()
    }

    fn dispatch_zone_fenced_teardown_followup(
        &mut self,
        command: ZoneCommand,
    ) -> Vec<ServerPacket> {
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
            let outbounds = zone_state.zone_manager.handle(command);
            zone_state.dispatch_zone_outbounds_with_fence_policy(outbounds, Some(&key), true)
        };
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&packets);
        self.inner.apply_shared_monster_lifecycle_packets(&packets);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets_by_object_id, canceled_claims) =
            self.apply_zone_ground_drop_claims(ground_drop_claims);
        merge_ground_drop_claim_packets_in_crystal_order(
            &mut packets,
            claim_packets_by_object_id,
            &canceled_claims,
        );
        packets
    }

    fn apply_zone_ground_drop_claims(
        &mut self,
        claims: Vec<GroundDropClaimTicket>,
    ) -> (BTreeMap<u32, Vec<ServerPacket>>, BTreeSet<u32>) {
        if claims.is_empty() {
            return (BTreeMap::new(), BTreeSet::new());
        }
        let active_identity = self.inner.active_identity();
        let current_session_id = self.current_zone_session_id();
        let mut packets_by_object_id = BTreeMap::<u32, Vec<ServerPacket>>::new();
        let mut canceled_claims = BTreeSet::new();
        for ticket in claims {
            let drop = ticket.drop.clone();
            let object_id = ticket.object_id;
            let identity_matches_claim = current_session_id.as_ref() == Some(&ticket.session_id);
            let outcome = match active_identity.as_ref().filter(|_| identity_matches_claim) {
                Some(identity) => {
                    self.commit_account_inventory_outcome(SharedAccountInventoryCommandEnvelope {
                        identity: identity.clone(),
                        command: SharedAccountInventoryCommand::GroundDropClaimPickup {
                            drop: drop.clone(),
                            claim_idempotency_key: ticket.idempotency_key.clone(),
                        },
                    })
                }
                None => SharedAccountInventoryCommitOutcome::Confirmed(
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
                        committed: false,
                        packets: Vec::new(),
                    },
                ),
            };
            let deferred = matches!(
                &outcome,
                SharedAccountInventoryCommitOutcome::Deferred { .. }
            );
            let outcome_unknown = match &outcome {
                SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                    idempotency_key,
                    execution_context,
                    ..
                } => Some((idempotency_key.clone(), execution_context.clone())),
                SharedAccountInventoryCommitOutcome::Confirmed(_)
                | SharedAccountInventoryCommitOutcome::Deferred { .. } => None,
            };
            let mut receipt = outcome.into_receipt();
            debug_assert_eq!(
                receipt.kind,
                SharedAccountInventoryTransactionKind::GroundDropPickup
            );
            let followup = if receipt.committed {
                self.retire_local_ground_drop_projection(&drop);
                Some(ZoneCommand::CommitGroundDropClaimWithTicket {
                    session_id: ticket.session_id.clone(),
                    ticket,
                })
            } else if let Some((idempotency_key, execution_context)) = outcome_unknown {
                // The database may already own this claim. Detach it from the
                // historical session while retaining the Zone tombstone and a
                // checkpointed account/character recovery authority.
                if !self.retain_unresolved_zone_ground_drop_claim(
                    &ticket,
                    idempotency_key,
                    Some(execution_context),
                ) {
                    self.requeue_zone_ground_drop_claim(ticket);
                }
                None
            } else if deferred {
                // No durable attempt occurred. Keep the Zone claim hidden and
                // retry it when a later ordered command provides a fence.
                self.requeue_zone_ground_drop_claim(ticket);
                None
            } else {
                self.restore_zone_ground_drop_claim(drop);
                canceled_claims.insert(object_id);
                Some(ZoneCommand::CancelGroundDropClaimWithTicket {
                    session_id: ticket.session_id.clone(),
                    ticket,
                    now_ms: Self::zone_now_ms(),
                })
            };
            if let Some(followup) = followup {
                let followup_packets = if self.teardown_is_fenced() {
                    self.dispatch_zone_fenced_teardown_followup(followup)
                } else {
                    self.dispatch_zone_player_command(followup, false)
                };
                receipt.packets.extend(followup_packets);
            }
            packets_by_object_id
                .entry(object_id)
                .or_default()
                .extend(receipt.packets);
        }
        (packets_by_object_id, canceled_claims)
    }
    fn retain_unresolved_zone_ground_drop_claim(
        &mut self,
        ticket: &GroundDropClaimTicket,
        idempotency_key: String,
        execution_context: Option<SharedAccountInventoryExecutionContext>,
    ) -> bool {
        let Some(key) = self.current_presence_key() else {
            return false;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .detach_unresolved_ground_drop_settlement(
                &key,
                ticket,
                idempotency_key,
                execution_context,
            )
    }

    fn requeue_zone_ground_drop_claim(&mut self, ticket: GroundDropClaimTicket) {
        let Some(key) = self.current_presence_key() else {
            return;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .queue_zone_ground_drop_claim(key, ticket);
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
        self.inner.local_player_object_id()
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
        let Some(current_key) = self.current_presence_key() else {
            return Vec::new();
        };
        let packets = {
            let zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let Some(map_file_name) = zone_state
                .players
                .get(&current_key)
                .map(|presence| presence.map_file_name.as_str())
            else {
                return Vec::new();
            };
            let zone_key = ZoneKey::for_map(map_file_name);
            packets
                .iter()
                .filter(|packet| {
                    // BroadcastPackets carries a personal SimulationSession's
                    // player-action projection. Its ObjectRemove can mean only
                    // that a static object left this one session's private
                    // viewport. Do not feed that visibility event back into
                    // the single-writer Zone as a world deletion. Player-owned
                    // summons remain eligible for their real lifecycle remove.
                    !matches!(
                        packet,
                        ServerPacket::ObjectRemove { object_id }
                            if zone_state
                                .shared_entity(map_file_name, *object_id)
                                .is_some_and(|entity| {
                                    entity.owner_name.is_none()
                                        && matches!(
                                            entity.kind,
                                            WorldEntityKind::Npc | WorldEntityKind::Monster
                                        )
                                })
                                && zone_state
                                    .zone_manager
                                    .zone(&zone_key)
                                    .is_some_and(|zone| zone.retains_object_id(*object_id))
                    )
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        if packets.is_empty() {
            return Vec::new();
        }
        self.dispatch_zone_player_command(
            ZoneCommand::BroadcastPackets {
                session_id,
                owner_local_object_id,
                packets,
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
                    // A personal SimulationSession also emits ObjectRemove
                    // when a static shared object leaves its private data
                    // range. Feeding that viewport packet back into the Zone
                    // would turn it into a real world deletion. Static NPCs
                    // and unowned native monsters are already single-writer
                    // Zone objects; only their Zone lifecycle may remove them.
                    let personal_viewport_remove = match packet {
                        ServerPacket::ObjectRemove { object_id } => zone_state
                            .shared_entity(&map_file_name, *object_id)
                            .is_some_and(|entity| {
                                entity.owner_name.is_none()
                                    && matches!(
                                        entity.kind,
                                        WorldEntityKind::Npc | WorldEntityKind::Monster
                                    )
                            }),
                        _ => false,
                    };
                    if personal_viewport_remove {
                        return false;
                    }
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
                    include_owner: false,
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
        &self,
        snapshot: &WorldSnapshot,
        zone_state: &SharedInProcessZoneState,
        current_key: Option<&ZonePresenceKey>,
    ) -> Vec<GroundDropSnapshot> {
        let map_file_name = snapshot.map_file_name.as_deref().unwrap_or_default();
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
        let mut drops = snapshot
            .ground_drops
            .iter()
            .filter_map(|drop| {
                let key = (map_file_name.to_string(), drop.object_id);
                if self.retired_local_ground_drop_ids.contains(&key) {
                    return None;
                }
                let mut drop = drop.clone();
                if let Some(zone_object_id) = self.local_ground_drop_zone_ids.get(&key) {
                    drop.object_id = *zone_object_id;
                } else if zone_state.maps.get(map_file_name).is_some_and(|map| {
                    map.ground_drops
                        .values()
                        .any(|shared| same_ground_drop_projection(shared, &drop))
                }) {
                    // A promoted replica restores the private character
                    // runtime separately from the shared Zone checkpoint.
                    // If the unique Zone drop is already present, do not
                    // re-introduce its old session-local object id.
                    return None;
                }
                Some(drop)
            })
            .collect::<Vec<_>>();
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

    fn remap_player_ground_drop_packets(&mut self, packets: &mut [ServerPacket]) {
        let Some(map_file_name) = self.inner.world_snapshot().map_file_name else {
            return;
        };
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        for packet in packets {
            let local_object_id = match packet {
                ServerPacket::ObjectGold { info } => info.object_id,
                ServerPacket::ObjectItem { info } => info.object_id,
                _ => continue,
            };
            let key = (map_file_name.clone(), local_object_id);
            self.retired_local_ground_drop_ids.remove(&key);
            let zone_object_id = if let Some(zone_object_id) =
                self.local_ground_drop_zone_ids.get(&key)
            {
                *zone_object_id
            } else {
                let zone_object_id = zone_state.next_zone_object_id;
                zone_state.next_zone_object_id = zone_state.next_zone_object_id.saturating_add(1);
                self.local_ground_drop_zone_ids.insert(key, zone_object_id);
                zone_object_id
            };
            match packet {
                ServerPacket::ObjectGold { info } => info.object_id = zone_object_id,
                ServerPacket::ObjectItem { info } => info.object_id = zone_object_id,
                _ => {}
            }
        }
    }

    fn retire_local_ground_drop_projection(&mut self, shared_drop: &GroundDropSnapshot) {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name else {
            return;
        };
        let Some(local_object_id) = snapshot
            .ground_drops
            .iter()
            .find(|drop| same_ground_drop_projection(drop, shared_drop))
            .map(|drop| drop.object_id)
        else {
            return;
        };
        let key = (map_file_name, local_object_id);
        self.local_ground_drop_zone_ids.remove(&key);
        self.retired_local_ground_drop_ids.insert(key);
    }

    fn apply_shared_entity_packets_to_current_map(&mut self, packets: &[ServerPacket]) {
        if packets.is_empty() {
            return;
        }
        let map_file_name = self.inner.world_snapshot().map_file_name;
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
            // The command tail includes Zone-generated AOI diffs as well as
            // true shared-world lifecycle packets. `ObjectRemove` only means a
            // global deletion when the single-writer Zone no longer retains the
            // object; otherwise it is local viewport bookkeeping. Filtering at
            // this second map-layer application boundary is essential because
            // the initial Zone outbound dispatch already made the same
            // distinction before the aggregate packet list reached us again.
            let zone_key = ZoneKey::for_map(map_file_name);
            let packets = packets
                .iter()
                .filter(|packet| {
                    !matches!(
                        packet,
                        ServerPacket::ObjectRemove { object_id }
                            if zone_state
                                .zone_manager
                                .zone(&zone_key)
                                .is_some_and(|zone| zone.retains_object_id(*object_id))
                    )
                })
                .map(|packet| match packet {
                    ServerPacket::ObjectMonster { info }
                        if local_self_object_id.is_some_and(|local_self_object_id| {
                            info.master_object_id == local_self_object_id
                        }) && zone_object_id.is_some() =>
                    {
                        let mut info = info.clone();
                        info.master_object_id = zone_object_id.expect("checked above");
                        ServerPacket::ObjectMonster { info }
                    }
                    packet => packet.clone(),
                })
                .collect::<Vec<_>>();
            zone_state.apply_shared_entity_packets(map_file_name, &packets);
        }
    }

    /// Personal `SimulationSession` ticks still advance private systems, but
    /// shared monsters are driven by the single Zone owner cadence. Dropping
    /// their private motion packets here prevents the same monster from being
    /// moved and broadcast a second time by every connected session.
    fn suppress_personal_tick_shared_monster_motion(&self, packets: &mut Vec<ServerPacket>) {
        let Some(map_file_name) = self.inner.world_snapshot().map_file_name else {
            return;
        };
        let shared_monster_ids = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .maps
            .get(&map_file_name)
            .map(|map| {
                map.entities
                    .values()
                    .filter(|entity| entity.kind == WorldEntityKind::Monster)
                    .map(|entity| entity.object_id)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        suppress_personal_tick_shared_monster_motion(packets, &shared_monster_ids);
    }

    fn commit_shared_death_drops_to_current_map(
        &mut self,
        packets: &[ServerPacket],
    ) -> Vec<ServerPacket> {
        if packets.is_empty() {
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
        let ground_drops = self.current_snapshot_ground_drops_for_shared_state(
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
        let (mut object_id, packet_direction, mut kind) = match command {
            WorldCommand::Attack { object_id } => (
                *object_id,
                None,
                ZoneNativePlayerAttackKind::Melee {
                    spell: Spell::None as u8,
                    attack_type: 0,
                },
            ),
            WorldCommand::ClientPacket(ClientPacket::Attack { direction, spell }) => (
                0,
                Some(*direction),
                ZoneNativePlayerAttackKind::Melee {
                    spell: *spell as u8,
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
                ZoneNativePlayerAttackKind::Magic {
                    target: location.clone(),
                    spell: *spell,
                    cast: true,
                    mp_cost: 0,
                    cooldown_ms: 0,
                    item_param: 0,
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
                ZoneNativePlayerAttackKind::Magic {
                    target: location.clone(),
                    spell: *spell,
                    cast: true,
                    mp_cost: 0,
                    cooldown_ms: 0,
                    item_param: 0,
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
                    ZoneNativePlayerAttackKind::Magic {
                        target: location.clone(),
                        spell: *spell,
                        cast: true,
                        mp_cost: 0,
                        cooldown_ms: 0,
                        item_param: 0,
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
                ZoneNativePlayerAttackKind::Magic {
                    target: location.clone(),
                    spell: *spell,
                    cast: true,
                    mp_cost: 0,
                    cooldown_ms: 0,
                    item_param: 0,
                },
            ),
            _ => return None,
        };
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.as_deref()?;
        if object_id == 0 {
            if let ZoneNativePlayerAttackKind::Melee { spell, .. } = &kind {
                let origin = self.authoritative_self_entity_for_snapshot(&snapshot)?;
                let requested_spell = Spell::try_from(*spell).unwrap_or(Spell::None);
                let max_distance = if requested_spell == Spell::Thrusting {
                    2
                } else {
                    1
                };
                object_id = (1..=max_distance).find_map(|distance| {
                    let mut point = Point {
                        x: origin.x,
                        y: origin.y,
                    };
                    for _ in 0..distance {
                        point = point_in_direction(&point, packet_direction?);
                    }
                    snapshot
                        .entities
                        .iter()
                        .find(|entity| {
                            !entity.dead
                                && matches!(
                                    entity.kind,
                                    WorldEntityKind::Monster | WorldEntityKind::Player
                                )
                                && entity.x == point.x
                                && entity.y == point.y
                        })
                        .map(|entity| entity.object_id)
                })?;
            }
        }
        let (monster, direction, is_player_target, is_red_player_target) = if object_id == 0 {
            (None, packet_direction, false, false)
        } else {
            let (shared_target, shared_pk_points) = {
                let zone_state = self
                    .zone_state
                    .lock()
                    .expect("shared zone presence mutex should not be poisoned");
                (
                    zone_state.shared_entity(map_file_name, object_id),
                    zone_state.shared_player_pk_points(map_file_name, object_id),
                )
            };
            let target = shared_target.or_else(|| {
                snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.object_id == object_id)
                    .cloned()
            });
            let target = target?;
            if !matches!(
                target.kind,
                WorldEntityKind::Monster | WorldEntityKind::Player
            ) || (target.dead
                && !matches!(
                    &kind,
                    ZoneNativePlayerAttackKind::Magic {
                        spell: Spell::Reincarnation,
                        ..
                    }
                ))
            {
                return None;
            }
            // Keep stale client coordinates inside the authoritative Zone
            // request. The Zone validates cooldown, range, and the live target
            // position in that order and returns a correction when required;
            // rejecting here would fall back to the private Session runtime.
            let monster = (target.kind == WorldEntityKind::Monster)
                .then(|| {
                    self.inner
                        .zone_monster_spawn_snapshot(object_id)
                        .or_else(|| {
                            zone_monster_spawn_from_shared_entity(
                                &target,
                                Self::zone_now_ms() / SHARED_CRYSTAL_TICK_MS,
                            )
                        })
                })
                .flatten();
            let direction = packet_direction.or_else(|| {
                // Low-latency movement advances the shared Zone before the
                // personal Session mirror. Derive implicit melee direction
                // from the authoritative presence, otherwise adjacent attacks
                // can be aimed from an old tile and silently fail Zone range.
                let self_entity = self.authoritative_self_entity_for_snapshot(&snapshot)?;
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
            let is_player_target = target.kind == WorldEntityKind::Player;
            let is_red_player_target =
                is_player_target && shared_pk_points.is_some_and(|pk_points| pk_points >= 100);
            (monster, direction, is_player_target, is_red_player_target)
        };
        let direction = direction?;
        let (level, damage) = match &mut kind {
            ZoneNativePlayerAttackKind::Melee { spell, .. } => {
                let requested_spell = Spell::try_from(*spell).unwrap_or(Spell::None);
                let (profile_spell, profile_level, profile_damage) =
                    self.inner.zone_melee_attack_profile(requested_spell);
                *spell = profile_spell as u8;
                (profile_level, profile_damage.max(1))
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
                item_param,
                ..
            } => {
                let (profile_level, profile_damage, profile_mp_cost, profile_cooldown_ms) =
                    self.inner.zone_magic_attack_profile(*spell)?;
                *mp_cost = profile_mp_cost;
                *cooldown_ms = profile_cooldown_ms;
                *item_param = self.inner.shared_skill_item_param(*spell);
                (profile_level, profile_damage.max(0))
            }
        };

        Some(ZoneNativePlayerAttack {
            object_id,
            is_player_target,
            is_red_player_target,
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
        let is_player_target = attack.is_player_target;
        let is_red_player_target = attack.is_red_player_target;
        let target_object_id = attack.object_id;
        let Some(session_id) = self.current_zone_session_id() else {
            return Vec::new();
        };
        let trusted_physical_monster_target = match &attack.kind {
            ZoneNativePlayerAttackKind::Melee { .. } => attack
                .monster
                .as_ref()
                .is_some_and(ZoneMonsterSpawn::is_authoritatively_melee_attackable_by_player),
            ZoneNativePlayerAttackKind::Range { .. } => attack
                .monster
                .as_ref()
                .is_some_and(ZoneMonsterSpawn::is_authoritatively_hostile_to_player),
            ZoneNativePlayerAttackKind::Magic { .. } => true,
        };
        if matches!(
            &attack.kind,
            ZoneNativePlayerAttackKind::Melee { .. } | ZoneNativePlayerAttackKind::Range { .. }
        ) && !is_player_target
            && !trusted_physical_monster_target
        {
            return self.authoritative_zone_owner_correction();
        }
        let now_ms = Self::zone_now_ms();
        let mut packets = if matches!(
            &attack.kind,
            ZoneNativePlayerAttackKind::Melee { .. } | ZoneNativePlayerAttackKind::Range { .. }
        ) {
            let Some(packets) = self.sync_authoritative_zone_combat_state(&session_id) else {
                return self.authoritative_zone_owner_correction();
            };
            packets
        } else {
            Vec::new()
        };
        let materialized_monster = attack.monster.take();
        if let ZoneNativePlayerAttackKind::Magic { spell, .. } = &attack.kind {
            // Magic remains on its existing lifecycle. Only melee/range use
            // the new admission+materialization atomic Zone transaction.
            if let Some(monster) = materialized_monster.as_ref() {
                packets.extend(self.dispatch_zone_player_command(
                    ZoneCommand::SpawnMonster {
                        session_id: session_id.clone(),
                        monster: monster.clone(),
                        now_ms,
                    },
                    false,
                ));
            }
            if gateway_zone_magic_requires_item_preflight_only(*spell)
                && self
                    .inner
                    .shared_skill_item_consumption_components(*spell)
                    .is_none()
            {
                return Vec::new();
            }
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
                let Some(components) = self.inner.shared_skill_item_consumption_components(*spell)
                else {
                    return Vec::new();
                };
                let request_id = self.next_shared_economy_request_id();
                let receipt =
                    self.commit_account_inventory(SharedAccountInventoryCommandEnvelope {
                        identity,
                        command: SharedAccountInventoryCommand::SkillItemConsume {
                            spell: *spell,
                            request_id,
                            components,
                        },
                    });
                if !receipt.committed {
                    return Vec::new();
                }
                packets.extend(receipt.packets);
            }
        }
        let mut accepted_magic_spend = None;
        let melee_spell_to_commit = match &attack.kind {
            ZoneNativePlayerAttackKind::Melee { spell, .. } => Spell::try_from(*spell).ok(),
            _ => None,
        };
        let command = match attack.kind {
            ZoneNativePlayerAttackKind::Melee { spell, attack_type } => {
                if is_player_target {
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
                } else {
                    ZoneCommand::PlayerAttackMaterializedObject {
                        session_id,
                        object_id: attack.object_id,
                        monster: materialized_monster,
                        direction: attack.direction,
                        spell,
                        level: attack.level,
                        attack_type,
                        damage: attack.damage,
                        now_ms,
                    }
                }
            }
            ZoneNativePlayerAttackKind::Range {
                target,
                spell,
                attack_type,
            } => {
                if is_player_target {
                    ZoneCommand::PlayerRangeAttackObject {
                        session_id,
                        object_id: attack.object_id,
                        direction: attack.direction,
                        target,
                        spell,
                        level: attack.level,
                        attack_type,
                        damage: attack.damage,
                        now_ms,
                    }
                } else {
                    ZoneCommand::PlayerRangeAttackMaterializedObject {
                        session_id,
                        object_id: attack.object_id,
                        monster: materialized_monster,
                        direction: attack.direction,
                        target,
                        spell,
                        level: attack.level,
                        attack_type,
                        damage: attack.damage,
                        now_ms,
                    }
                }
            }
            ZoneNativePlayerAttackKind::Magic {
                target,
                spell,
                cast,
                mp_cost,
                cooldown_ms,
                item_param,
            } => {
                accepted_magic_spend = cast.then_some((spell, mp_cost, cooldown_ms));
                ZoneCommand::PlayerCastMagicWithItem {
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
                    item_param,
                    now_ms,
                }
            }
        };
        let mut dispatched = self.dispatch_zone_player_command(command, false);
        if let Some(spell) = melee_spell_to_commit.filter(|spell| *spell != Spell::None) {
            let accepted = dispatched.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectAttack { info }
                        if info.object_id != target_object_id && info.spell == spell as u8
                )
            });
            if accepted {
                dispatched.extend(self.inner.commit_zone_melee_attack_spell(spell));
            }
        }
        let mut pk_colour_changed = false;
        if is_player_target
            && dispatched.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectDied { info } if info.object_id == target_object_id
                )
            })
        {
            let attack_mode = self.inner.world_snapshot().stage5_systems.attack_mode;
            if !is_red_player_target && !matches!(attack_mode, 3 | 4) {
                self.inner.apply_zone_unlawful_player_kill(100);
                pk_colour_changed = true;
            }
        }
        if let Some((spell, mp_cost, cooldown_ms)) = accepted_magic_spend {
            if zone_magic_launch_accepted(&dispatched, spell, attack.object_id) {
                self.inner
                    .apply_zone_player_magic_spend(spell, mp_cost, cooldown_ms);
            }
        }
        packets.extend(dispatched);
        if pk_colour_changed {
            let colour_packet = ServerPacket::ColourChanged {
                name_colour_argb: self.inner.zone_player_name_colour_argb(),
            };
            if let Some(owner_local_object_id) = self.local_self_object_id() {
                packets.extend(self.dispatch_zone_observer_packets(
                    owner_local_object_id,
                    std::slice::from_ref(&colour_packet),
                ));
            }
            packets.push(colour_packet);
        }
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
            ..
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

    fn apply_shared_harvest_target_to_local(&mut self, direction: MirDirection) -> bool {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return false;
        };
        let Some(self_entity) = self.authoritative_self_entity_for_snapshot(&snapshot) else {
            return false;
        };
        let target = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .shared_harvest_target_snapshot(map_file_name, &self_entity, direction);
        target.is_some_and(|entity| self.inner.apply_shared_entity_snapshot(&entity))
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

    fn execute_zone_npc_teleport(
        &mut self,
        session_id: SessionId,
        object_id: u32,
    ) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let Some(mut checkpoint) = self.inner.active_character_checkpoint() else {
            return Vec::new();
        };
        let available_gold = self.inner.world_snapshot().gold;
        if checkpoint.gold != available_gold {
            return Vec::new();
        }

        let zone_state_handle = Arc::clone(&self.zone_state);
        let mut zone_state = zone_state_handle
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        let Some(old_transform) = zone_state.zone_manager.player_transform(&session_id) else {
            return Vec::new();
        };
        let mut outbounds = zone_state.zone_manager.handle(ZoneCommand::TeleportToNpc {
            session_id: session_id.clone(),
            object_id,
            available_gold,
        });
        let commit = outbounds.iter().find_map(|outbound| match outbound {
            ZoneOutbound::NpcTeleportCommit {
                session_id: committed_session,
                gold_cost,
                map,
            } if committed_session == &session_id => Some((*gold_cost, map.clone())),
            _ => None,
        });
        let Some((gold_cost, map)) = commit else {
            return Vec::new();
        };
        let Some(proposed_transform) = outbounds.iter().find_map(|outbound| match outbound {
            ZoneOutbound::SaveTransform {
                session_id: transformed_session,
                position,
                direction,
            } if transformed_session == &session_id => Some((position.clone(), *direction)),
            _ => None,
        }) else {
            let _ = zone_state
                .zone_manager
                .handle(ZoneCommand::SyncPlayerTransform {
                    session_id,
                    position: old_transform.0,
                    direction: old_transform.1,
                });
            return Vec::new();
        };
        let Some(remaining_gold) = checkpoint.gold.checked_sub(gold_cost) else {
            let _ = zone_state
                .zone_manager
                .handle(ZoneCommand::SyncPlayerTransform {
                    session_id,
                    position: old_transform.0,
                    direction: old_transform.1,
                });
            return Vec::new();
        };
        checkpoint.gold = remaining_gold;
        #[cfg(test)]
        let force_checkpoint_failure =
            std::mem::take(&mut self.fail_next_npc_teleport_checkpoint_restore);
        #[cfg(not(test))]
        let force_checkpoint_failure = false;
        if force_checkpoint_failure
            || self
                .inner
                .restore_active_character_checkpoint(&checkpoint)
                .is_err()
        {
            let _ = zone_state
                .zone_manager
                .handle(ZoneCommand::SyncPlayerTransform {
                    session_id,
                    position: old_transform.0,
                    direction: old_transform.1,
                });
            return Vec::new();
        }
        outbounds.retain(|outbound| !matches!(outbound, ZoneOutbound::NpcTeleportCommit { .. }));
        let (zone_packets, transform, _, _, _, _, _) =
            zone_state.dispatch_zone_outbounds(outbounds, Some(&key));
        drop(zone_state);

        debug_assert_eq!(transform, Some(proposed_transform.clone()));
        let (position, direction) = transform.unwrap_or(proposed_transform);
        self.apply_zone_transform(Some((position.clone(), direction)));
        let mut packets = vec![
            ServerPacket::LoseGold { gold: gold_cost },
            ServerPacket::MapChanged {
                map_index: map.map_index,
                file_name: map.file_name,
                title: map.title,
                mini_map: map.mini_map,
                big_map: map.big_map,
                lights: map.lights,
                location: position.clone(),
                direction,
                map_dark_light: map.map_dark_light,
                music: map.music,
                weather: map.weather,
            },
            ServerPacket::UserLocation {
                location: UserLocation {
                    position,
                    direction,
                },
            },
        ];
        packets.extend(zone_packets);
        packets
    }

    fn execute_zone_player_packet(&mut self, packet: &ClientPacket) -> Option<Vec<ServerPacket>> {
        let session_id = self.current_zone_session_id()?;
        match packet {
            ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. } => {
                let execution = execute_shared_zone_movement(
                    &self.zone_state,
                    &self.movement_ingress.session_state,
                    packet,
                    None,
                    false,
                    false,
                )
                .expect("shared zone movement execution should not fail");
                let Some(execution) = execution else {
                    // Preserve the existing shared-zone guard: an active session with
                    // stale/missing presence must not mutate its private world instead.
                    return Some(Vec::new());
                };
                self.apply_zone_transform(execution.transform);
                let mut packets = execution.packets;
                if matches!(packet, ClientPacket::Walk { .. } | ClientPacket::Run { .. }) {
                    packets.extend(self.sync_newly_active_private_monsters_to_zone());
                }
                // Crystal Turn invokes CheckMovement on the standing tile after
                // applying the requested direction.  Walk/Run and Turn must all
                // hand an admitted MapCoord transfer back to the personal
                // session so Gateway can commit the real map change/rebind.
                packets.extend(self.apply_zone_current_position_map_transfer());
                Some(packets)
            }
            ClientPacket::TeleportToNpc { object_id } => {
                Some(self.execute_zone_npc_teleport(session_id, *object_id))
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
            ClientPacket::AcceptReincarnation | ClientPacket::CancelReincarnation => {
                Some(self.dispatch_zone_player_command(
                    ZoneCommand::ResolveReincarnation {
                        session_id,
                        accept: matches!(packet, ClientPacket::AcceptReincarnation),
                        now_ms: Self::zone_now_ms(),
                    },
                    false,
                ))
            }
            _ => None,
        }
    }

    fn reconcile_durable_ground_drop_projections(&mut self) -> Vec<ServerPacket> {
        let identity = self.inner.active_identity();
        if identity.is_none()
            || identity == self.last_ground_drop_projection_reconciliation_identity
        {
            return Vec::new();
        }
        let service = Arc::clone(&self.account_inventory_service);
        let context = self.economy_execution_context.clone();
        let packets =
            service.reconcile_ground_drop_projections_fenced(&mut self.inner, context.as_ref());
        if !service.has_pending_ground_drop_projection_fenced(&self.inner, context.as_ref()) {
            self.last_ground_drop_projection_reconciliation_identity = identity;
        }
        packets
    }

    fn note_durable_trade_projection_pending(&mut self) {
        self.trade_projection_reconciliation_state = self
            .inner
            .active_identity()
            .map(TradeProjectionReconciliationState::Pending)
            .unwrap_or(TradeProjectionReconciliationState::Unknown);
    }

    fn reconcile_durable_trade_projections(&mut self) -> Vec<ServerPacket> {
        let Some(identity) = self.inner.active_identity() else {
            self.trade_projection_reconciliation_state =
                TradeProjectionReconciliationState::Unknown;
            return Vec::new();
        };
        // Unknown/Pending states always re-query. Only a confirmed Clear state
        // for this exact character may use the no-local-trade fast path.
        let should_reconcile = match &self.trade_projection_reconciliation_state {
            TradeProjectionReconciliationState::Clear(cached_identity)
                if cached_identity == &identity =>
            {
                self.inner.has_active_shared_trade_state()
            }
            TradeProjectionReconciliationState::Unknown
            | TradeProjectionReconciliationState::Clear(_)
            | TradeProjectionReconciliationState::Pending(_) => true,
        };
        if !should_reconcile {
            return Vec::new();
        }
        let service = Arc::clone(&self.account_inventory_service);
        let context = self.economy_execution_context.clone();
        let packets = service.reconcile_trade_projections_fenced(&mut self.inner, context.as_ref());
        self.trade_projection_reconciliation_state =
            if service.has_pending_trade_projection_fenced(&self.inner, context.as_ref()) {
                TradeProjectionReconciliationState::Pending(identity)
            } else {
                TradeProjectionReconciliationState::Clear(identity)
            };
        packets
    }

    fn has_pending_durable_trade_projection(&self) -> bool {
        if self.has_unresolved_trade_settlement() {
            return true;
        }
        let Some(identity) = self.inner.active_identity() else {
            return false;
        };
        match &self.trade_projection_reconciliation_state {
            TradeProjectionReconciliationState::Unknown
            | TradeProjectionReconciliationState::Pending(_) => true,
            TradeProjectionReconciliationState::Clear(cached_identity)
                if cached_identity != &identity =>
            {
                true
            }
            TradeProjectionReconciliationState::Clear(_) => {
                self.inner.has_active_shared_trade_state()
                    && self
                        .account_inventory_service
                        .has_pending_trade_projection_fenced(
                            &self.inner,
                            self.economy_execution_context.as_ref(),
                        )
            }
        }
    }

    /// Retry one detached ground-drop settlement for the same account and
    /// character. Unknown stays hidden; a durable commit retires the recovery
    /// record, while only a definitive rejection restores the exact Zone drop.
    fn resolve_unresolved_ground_drop_settlement(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let settlement = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .unresolved_ground_drop_settlement_for_presence(&key);
        let Some(settlement) = settlement else {
            return Vec::new();
        };
        let Some(identity) = self.inner.active_identity() else {
            return Vec::new();
        };
        if ZonePresenceKey::from_identity(&identity) != settlement.presence_key {
            return Vec::new();
        }
        if !self
            .economy_execution_context
            .as_ref()
            .is_some_and(|context| context.external_commit_authorized)
        {
            return Vec::new();
        }
        let Some(expected_idempotency_key) = settlement.idempotency_key.as_deref() else {
            return Vec::new();
        };
        let Some(recovery_context) = settlement.execution_context.as_ref() else {
            return Vec::new();
        };
        let outcome = self.retry_account_inventory_outcome(
            recovery_context,
            expected_idempotency_key,
            SharedAccountInventoryCommandEnvelope {
                identity,
                command: SharedAccountInventoryCommand::GroundDropClaimPickup {
                    drop: settlement.ticket.drop.clone(),
                    claim_idempotency_key: settlement.ticket.idempotency_key.clone(),
                },
            },
        );
        let receipt = match outcome {
            SharedAccountInventoryCommitOutcome::Deferred { .. } => return Vec::new(),
            SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                idempotency_key,
                execution_context,
                ..
            } => {
                if idempotency_key != expected_idempotency_key
                    || execution_context != *recovery_context
                {
                    return Vec::new();
                }
                self.zone_state
                    .lock()
                    .expect("shared zone presence mutex should not be poisoned")
                    .retain_unresolved_ground_drop_outcome_key(&settlement, idempotency_key);
                return Vec::new();
            }
            SharedAccountInventoryCommitOutcome::Confirmed(receipt) => receipt,
        };
        if receipt.committed {
            self.retire_local_ground_drop_projection(&settlement.ticket.drop);
        }
        let dispatched = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let Some(outbounds) = zone_state.resolve_unresolved_ground_drop_settlement(
                &settlement,
                receipt.committed,
                Self::zone_now_ms(),
            ) else {
                return Vec::new();
            };
            zone_state.dispatch_zone_outbounds(outbounds, Some(&key))
        };
        let (
            mut zone_packets,
            transform,
            shout_consume,
            ground_drop_claims,
            monster_kill_awards,
            player_damages,
            player_heals,
        ) = dispatched;
        self.apply_zone_transform(transform);
        self.apply_zone_shout_consume(shout_consume);
        zone_packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        self.apply_zone_player_buff_packets(&zone_packets);
        self.inner
            .apply_shared_monster_lifecycle_packets(&zone_packets);
        zone_packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        if !ground_drop_claims.is_empty() {
            let (claim_packets, canceled_claims) =
                self.apply_zone_ground_drop_claims(ground_drop_claims);
            merge_ground_drop_claim_packets_in_crystal_order(
                &mut zone_packets,
                claim_packets,
                &canceled_claims,
            );
        }
        let mut packets = receipt.packets;
        packets.extend(zone_packets);
        packets
    }

    fn has_unresolved_trade_settlement(&self) -> bool {
        let Some(key) = self.current_presence_key() else {
            return false;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .has_unresolved_trade_settlement_for_presence(&key)
    }

    /// Retry one retained commit-ack-unknown trade with the exact same offers.
    /// The shared checkpoint record is removed only after a definitive durable,
    /// local-development, or rejected result has been observed.
    fn resolve_unresolved_trade_settlement(&mut self) -> bool {
        let Some(key) = self.current_presence_key() else {
            return false;
        };
        let settlement = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .unresolved_trade_settlement_for_presence(&key);
        let Some(settlement) = settlement else {
            return false;
        };
        if !self
            .economy_execution_context
            .as_ref()
            .is_some_and(|context| context.external_commit_authorized)
        {
            return true;
        }
        let Some(recovery_context) = settlement.execution_context.as_ref() else {
            return true;
        };
        let outcome = self.retry_shared_trade(
            recovery_context,
            &settlement.idempotency_key,
            &settlement.first_offer,
            &settlement.second_offer,
        );
        let resolution = match outcome {
            SharedTradeSettlementOutcome::Deferred => {
                // The prior uncertain attempt still owns both offers. Missing
                // context on this pass cannot resolve or roll it back.
                self.note_durable_trade_projection_pending();
                None
            }
            SharedTradeSettlementOutcome::DurableCommitted { .. }
            | SharedTradeSettlementOutcome::DurableDuplicate { .. } => {
                self.note_durable_trade_projection_pending();
                Some(UnresolvedSharedTradeResolution::Durable)
            }
            SharedTradeSettlementOutcome::Committed | SharedTradeSettlementOutcome::Duplicate => {
                self.trade_projection_reconciliation_state =
                    TradeProjectionReconciliationState::Unknown;
                Some(UnresolvedSharedTradeResolution::LocalCommit)
            }
            SharedTradeSettlementOutcome::Rejected => {
                self.trade_projection_reconciliation_state =
                    TradeProjectionReconciliationState::Unknown;
                Some(UnresolvedSharedTradeResolution::Rejected)
            }
            SharedTradeSettlementOutcome::OutcomeUnknown {
                idempotency_key,
                execution_context,
            } => {
                if idempotency_key != settlement.idempotency_key
                    || execution_context != *recovery_context
                {
                    return true;
                }
                self.note_durable_trade_projection_pending();
                None
            }
        };
        if let Some(resolution) = resolution {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .resolve_unresolved_trade_settlement(&settlement, resolution);
        }
        self.has_unresolved_trade_settlement()
    }

    fn apply_pending_shared_trade_packets(&mut self) -> Vec<ServerPacket> {
        let mut packets = self.resolve_unresolved_ground_drop_settlement();
        packets.extend(self.reconcile_durable_ground_drop_projections());
        let unresolved_trade = self.resolve_unresolved_trade_settlement();
        if unresolved_trade {
            self.note_durable_trade_projection_pending();
        } else {
            packets.extend(self.reconcile_durable_trade_projections());
        }
        packets.extend(self.apply_finalized_shared_trade_packets());
        packets
    }

    /// Materialize only trade deliveries/rollbacks whose settlement outcome is
    /// already final. This performs no external lookup or commit and is safe in
    /// teardown/Drop paths that intentionally lack an economy command context.
    fn apply_finalized_shared_trade_packets(&mut self) -> Vec<ServerPacket> {
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

    fn cancel_pending_shared_trade_offers_for_character(
        &mut self,
        character_name: Option<&str>,
    ) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let character_name = character_name.map(str::to_owned).unwrap_or_else(|| {
            self.inner
                .active_identity()
                .map(|identity| identity.character_name)
                .unwrap_or_default()
        });
        let own_offer = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .cancel_trade_offers_for_presence(&key, &character_name);
        own_offer
            .map(|offer| self.inner.rollback_shared_trade_offer(&offer))
            .unwrap_or_default()
    }

    fn cancel_pending_shared_trade_offers(&mut self) -> Vec<ServerPacket> {
        self.cancel_pending_shared_trade_offers_for_character(None)
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

        if !self.bootstrap_account_inventory() {
            return self.inner.shared_trade_cancel(false);
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

        let mut matched_offer = None;
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
                        matched_offer = Some((partner_key, partner_offer));
                    } else {
                        zone_state
                            .pending_trade_rollbacks
                            .entry(partner_key)
                            .or_default()
                            .push(partner_offer);
                        rollback_self = Some(offer.clone());
                    }
                } else {
                    zone_state
                        .trade_offers
                        .insert(self_key.clone(), offer.clone());
                }
            } else {
                rollback_self = Some(offer.clone());
            }
        }

        let mut deliver_to_self = Vec::new();
        let mut durable_settlement = false;
        let mut unknown_settlement = false;
        if let Some((partner_key, partner_offer)) = matched_offer {
            let settlement = self.settle_shared_trade(&offer, &partner_offer);
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            match settlement {
                SharedTradeSettlementOutcome::Committed
                | SharedTradeSettlementOutcome::Duplicate => {
                    zone_state
                        .pending_trade_deliveries
                        .entry(partner_key)
                        .or_default()
                        .push(offer.clone());
                    deliver_to_self.push(partner_offer);
                }
                SharedTradeSettlementOutcome::DurableCommitted { .. }
                | SharedTradeSettlementOutcome::DurableDuplicate { .. } => {
                    durable_settlement = true;
                }
                SharedTradeSettlementOutcome::OutcomeUnknown {
                    idempotency_key,
                    execution_context,
                } => {
                    let unresolved = UnresolvedSharedTradeSettlement {
                        idempotency_key,
                        execution_context: Some(execution_context),
                        first_key: self_key.clone(),
                        second_key: partner_key,
                        first_offer: offer.clone(),
                        second_offer: partner_offer,
                    };
                    // A conflicting cryptographic idempotency key must still
                    // fail closed: retain the existing recovery authority and
                    // keep this character's trade gate Pending.
                    let _ = zone_state.retain_unresolved_trade_settlement(unresolved);
                    unknown_settlement = true;
                }
                SharedTradeSettlementOutcome::Deferred | SharedTradeSettlementOutcome::Rejected => {
                    // This is the initial attempt and Deferred guarantees that
                    // no store call occurred, so returning both offers is safe.
                    zone_state
                        .pending_trade_rollbacks
                        .entry(partner_key)
                        .or_default()
                        .push(partner_offer);
                    rollback_self = Some(offer.clone());
                }
            }
        }

        if let Some(offer) = rollback_self {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
        }
        for offer in deliver_to_self {
            packets.extend(self.inner.apply_shared_trade_delivery(&offer));
        }
        if unknown_settlement {
            // Neither side may regain or reuse the debited assets until the
            // exact same idempotent settlement produces a definitive result.
            self.note_durable_trade_projection_pending();
        }
        if durable_settlement {
            // The durable commit creates projection work after a previously
            // cached Clear result. Invalidate the fast path before attempting
            // immediate private reconciliation so save/mark failures remain
            // fail-closed on subsequent commands and ticks.
            self.note_durable_trade_projection_pending();
            packets.extend(self.reconcile_durable_trade_projections());
        }
        packets
    }

    fn pick_up_shared_drop(&mut self, object_id: Option<u32>) -> Option<Vec<ServerPacket>> {
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = self.authoritative_self_entity_for_snapshot(&snapshot) else {
            return None;
        };
        let Some(session_id) = self.current_zone_session_id() else {
            return None;
        };
        self.sync_current_shared_ground_drops_to_zone(&session_id);
        let picker_group_members = snapshot.stage5_systems.group.members.clone();
        Some(self.dispatch_zone_player_command(
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
        ))
    }

    fn pick_up_shared_drop_with_intelligent_creature(
        &mut self,
        location: Point,
        mouse_mode: bool,
    ) -> Vec<ServerPacket> {
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = self.authoritative_self_entity_for_snapshot(&snapshot) else {
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
        self.sync_current_shared_ground_drops_to_zone(&session_id);
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
        packets.extend(self.apply_zone_player_damages(player_damages));
        self.apply_zone_player_heals(player_heals);
        packets.extend(self.apply_zone_monster_kill_awards(monster_kill_awards));
        let (claim_packets, canceled_claims) =
            self.apply_shared_intelligent_creature_drop_claims(&active_creature, claims);
        remove_object_remove_packets(&mut packets, &canceled_claims);
        packets.extend(claim_packets);
        packets
    }

    fn auto_pick_up_shared_drop_with_intelligent_creature(&mut self) -> Vec<ServerPacket> {
        if !self.inner.has_active_intelligent_creature_auto_pickup() {
            return Vec::new();
        }
        let snapshot = self.inner.world_snapshot();
        let Some(self_entity) = self.authoritative_self_entity_for_snapshot(&snapshot) else {
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
        self.sync_current_shared_ground_drops_to_zone(&session_id);
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
        packets.extend(self.apply_zone_player_damages(player_damages));
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
        claims: Vec<GroundDropClaimTicket>,
    ) -> (Vec<ServerPacket>, BTreeSet<u32>) {
        let mut packets = Vec::new();
        let mut canceled_claims = BTreeSet::new();
        for ticket in claims {
            let (claim_packets, canceled_object_id) =
                self.apply_shared_intelligent_creature_drop_claim(active_creature, ticket);
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
        ticket: GroundDropClaimTicket,
    ) -> (Vec<ServerPacket>, Option<u32>) {
        let drop = ticket.drop.clone();
        let object_id = ticket.object_id;
        let session_matches_claim =
            self.current_zone_session_id().as_ref() == Some(&ticket.session_id);
        let eligible = session_matches_claim
            && intelligent_creature_allows_ground_drop(active_creature, &drop);
        let active_identity = self.inner.active_identity();
        let outcome = match active_identity.as_ref().filter(|_| eligible) {
            Some(identity) => {
                self.commit_account_inventory_outcome(SharedAccountInventoryCommandEnvelope {
                    identity: identity.clone(),
                    command: SharedAccountInventoryCommand::GroundDropClaimPickup {
                        drop: drop.clone(),
                        claim_idempotency_key: ticket.idempotency_key.clone(),
                    },
                })
            }
            None => SharedAccountInventoryCommitOutcome::Confirmed(
                SharedAccountInventoryTransactionReceipt {
                    kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
                    committed: false,
                    packets: Vec::new(),
                },
            ),
        };
        let deferred = matches!(
            &outcome,
            SharedAccountInventoryCommitOutcome::Deferred { .. }
        );
        let outcome_unknown = match &outcome {
            SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                idempotency_key,
                execution_context,
                ..
            } => Some((idempotency_key.clone(), execution_context.clone())),
            SharedAccountInventoryCommitOutcome::Confirmed(_)
            | SharedAccountInventoryCommitOutcome::Deferred { .. } => None,
        };
        let mut receipt = outcome.into_receipt();
        debug_assert_eq!(
            receipt.kind,
            SharedAccountInventoryTransactionKind::GroundDropPickup
        );
        let (followup, canceled_object_id) = if receipt.committed {
            self.retire_local_ground_drop_projection(&drop);
            receipt
                .packets
                .insert(0, ServerPacket::IntelligentCreaturePickup { object_id });
            (
                Some(ZoneCommand::CommitGroundDropClaimWithTicket {
                    session_id: ticket.session_id.clone(),
                    ticket,
                }),
                None,
            )
        } else if let Some((idempotency_key, execution_context)) = outcome_unknown {
            if !self.retain_unresolved_zone_ground_drop_claim(
                &ticket,
                idempotency_key,
                Some(execution_context),
            ) {
                self.requeue_zone_ground_drop_claim(ticket);
            }
            (None, None)
        } else if deferred {
            self.requeue_zone_ground_drop_claim(ticket);
            (None, None)
        } else {
            self.restore_zone_ground_drop_claim(drop);
            (
                Some(ZoneCommand::CancelGroundDropClaimWithTicket {
                    session_id: ticket.session_id.clone(),
                    ticket,
                    now_ms: Self::zone_now_ms(),
                }),
                Some(object_id),
            )
        };
        if let Some(followup) = followup {
            let followup_packets = if self.teardown_is_fenced() {
                self.dispatch_zone_fenced_teardown_followup(followup)
            } else {
                self.dispatch_zone_player_command(followup, false)
            };
            receipt.packets.extend(followup_packets);
        }
        (receipt.packets, canceled_object_id)
    }
    fn adjacent_remote_player_name(&self) -> Option<String> {
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.as_deref()?;
        let current_key = self.current_presence_key();
        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)?;
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .remote_player_entities(Some(map_file_name), current_key.as_ref())
            .into_iter()
            .find(|entity| {
                (entity.x - self_entity.x).abs() <= 1 && (entity.y - self_entity.y).abs() <= 1
            })
            .map(|entity| entity.name)
    }
}

impl Drop for SharedInProcessZoneSessionRuntime {
    fn drop(&mut self) {
        // Drop is outside an ordered/fenced command. Preserve unresolved
        // durable outcomes for checkpoint/new-login recovery and only apply
        // already-finalized local deliveries or rollbacks.
        let _ = self.apply_finalized_shared_trade_packets();
        let _ = self.cancel_pending_shared_trade_offers();
        self.remove_presence();
    }
}

impl WorldRuntime for SharedInProcessZoneSessionRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_connect(&self) -> Vec<ServerPacket> {
        self.inner.on_connect()
    }

    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.last_game_shop_purchase_outcome = None;
        if self.current_presence_key().is_some_and(|key| {
            self.zone_state
                .lock()
                .map(|state| state.teardown_fenced(&key))
                .unwrap_or(true)
        }) {
            return Err("shared Zone session is fenced for teardown".to_string());
        }
        let removes_presence = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::Disconnect | ClientPacket::LogOut)
        );
        let departing_character_name = removes_presence
            .then(|| {
                self.inner
                    .active_identity()
                    .map(|identity| identity.character_name)
            })
            .flatten();
        if removes_presence {
            self.sync_pending_zone_movement_transform()?;
        }
        let is_low_latency_zone_player_packet = matches!(
            &command,
            WorldCommand::ClientPacket(
                ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. }
            )
        );
        let is_personal_session_chat_packet = match &command {
            WorldCommand::ClientPacket(ClientPacket::Chat { message, .. }) => {
                let trimmed = message.trim_start();
                (trimmed.len() > 1
                    && trimmed.starts_with('@')
                    && !trimmed.starts_with("@!")
                    && !trimmed.eq_ignore_ascii_case("@ADDSTORAGE"))
                    || self.inner.gm_login_pending()
            }
            _ => false,
        };
        let is_low_latency_zone_chat_packet = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::Chat { .. })
        ) && !is_personal_session_chat_packet;
        let is_low_latency_zone_packet =
            is_low_latency_zone_player_packet || is_low_latency_zone_chat_packet;
        let is_start_game = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::StartGame { .. })
        );
        let is_transfer_map_command = matches!(&command, WorldCommand::TransferMap { .. });
        let is_authoritative_move_to = matches!(&command, WorldCommand::MoveTo { .. });
        let is_handoff_transform = matches!(&command, WorldCommand::ApplyHandoffTransform { .. });
        let is_town_revive = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::TownRevive)
        );
        // The browser can already have received the private Crystal death while
        // the shared Zone still retains its native-combat 1 HP floor. Snapshot
        // that presented death before any Zone reconciliation: TownRevive must
        // execute against it instead of silently turning the player alive just
        // before the packet handler checks `Dead`.
        let revives_presented_private_death =
            is_town_revive && self.inner.world_snapshot().player_hp == Some(0);
        let applies_native_state = matches!(
            &command,
            WorldCommand::Stage5Command { action, .. } if action == "qa.applyNativeState"
        );
        let is_world_tick = matches!(&command, WorldCommand::Tick);
        let is_game_shop_buy = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::GameShopBuy { .. })
                | WorldCommand::NativeGameShopPurchase(_)
        );
        let skip_tail_zone_snapshot = is_low_latency_zone_packet || is_world_tick;
        let forwards_delayed_player_action_packets = matches!(&command, WorldCommand::Tick);
        let is_trade_cancel = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::TradeCancel)
        );
        let is_trade_state_mutation = matches!(
            &command,
            WorldCommand::ClientPacket(
                ClientPacket::TradeRequest
                    | ClientPacket::TradeReply { .. }
                    | ClientPacket::TradeGold { .. }
                    | ClientPacket::DepositTradeItem { .. }
                    | ClientPacket::RetrieveTradeItem { .. }
                    | ClientPacket::TradeConfirm { .. }
                    | ClientPacket::TradeCancel
            )
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
        let shared_gold_drop_amount = match &command {
            WorldCommand::ClientPacket(ClientPacket::DropGold { amount }) => Some(*amount),
            _ => None,
        };
        let shared_inventory_item_drop = match &command {
            WorldCommand::ClientPacket(ClientPacket::DropItem {
                unique_id,
                count,
                hero_inventory,
            }) => Some((*unique_id, *count, *hero_inventory)),
            WorldCommand::DropItem { key } => self
                .inner
                .world_snapshot()
                .inventory_items
                .iter()
                .find(|item| item.key == *key)
                .map(|item| (item.unique_id, 1, false)),
            _ => None,
        };
        let syncs_inner_position_before_session_execute = matches!(
            &command,
            WorldCommand::PickUp { .. }
                | WorldCommand::DropItem { .. }
                | WorldCommand::Interact { .. }
                | WorldCommand::SelectNpcDialog { .. }
                | WorldCommand::SubmitNpcInput { .. }
                | WorldCommand::ClientPacket(
                    ClientPacket::PickUp
                        | ClientPacket::Harvest { .. }
                        | ClientPacket::DropGold { .. }
                        | ClientPacket::DropItem { .. }
                        | ClientPacket::CallNpc { .. }
                        | ClientPacket::NpcConfirmInput { .. }
                        | ClientPacket::AcceptQuest { .. }
                        | ClientPacket::FinishQuest { .. }
                )
        );
        let cancels_pending_zone_player_movement = matches!(
            &command,
            WorldCommand::Interact { .. }
                | WorldCommand::SelectNpcDialog { .. }
                | WorldCommand::SubmitNpcInput { .. }
                | WorldCommand::ClientPacket(
                    ClientPacket::CallNpc { .. }
                        | ClientPacket::NpcConfirmInput { .. }
                        | ClientPacket::AcceptQuest { .. }
                        | ClientPacket::FinishQuest { .. }
                )
        );
        // Shared action gates must observe the same authoritative transform as
        // the personal SimulationSession command they guard. Low-latency Zone
        // movement can leave the private mirror several tiles behind; checking
        // Harvest availability before this sync silently rejects a corpse that
        // is directly in front of the real player.
        if syncs_inner_position_before_session_execute {
            self.force_inner_to_current_zone_transform();
        }
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
        // Native Zone combat already updates the shared map through its
        // authoritative outbounds, so rebuilding the full local map snapshot
        // after every attack is redundant and serializes the hot path.
        let skip_tail_zone_snapshot = skip_tail_zone_snapshot || routes_zone_native_player_attack;
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
        let mut unavailable_shared_target = !routes_zone_native_player_attack
            && match &command {
                WorldCommand::Attack { object_id } => {
                    !self.shared_action_target_available(*object_id)
                }
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
        let shared_harvest_direction = match &command {
            WorldCommand::ClientPacket(ClientPacket::Harvest { direction }) => Some(*direction),
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
        let mut packets = if is_low_latency_zone_packet {
            Vec::new()
        } else {
            let mut packets = self.apply_pending_zone_packets();
            packets.extend(self.apply_pending_shared_trade_packets());
            packets.extend(self.apply_pending_shared_rental_packets());
            packets
        };
        if cancels_pending_zone_player_movement {
            packets.extend(self.cancel_pending_zone_player_movement());
        }
        if removes_presence {
            // Roll back an unmatched, already-debited offer before the inner
            // LogOut/Disconnect path persists and clears the active character.
            // Matched durable trades have already left `trade_offers` and are
            // recovered through their PostgreSQL projection rows instead.
            packets.extend(self.cancel_pending_shared_trade_offers_for_character(
                departing_character_name.as_deref(),
            ));
        }
        if !is_low_latency_zone_packet && !revives_presented_private_death {
            // Damage/heal outbounds are deltas used for client packets. The
            // shared Zone is the exact vitals authority, so reconcile after
            // consuming them; otherwise a delayed pre-revive damage delta can
            // kill only the private runtime and make the next item command
            // disagree with the Zone snapshot.
            self.force_inner_to_current_zone_vitals();
        }
        // Harvest is admitted by the shared Zone from freshly synchronized,
        // trusted session state before any corpse is materialized in the
        // private compatibility runtime. Missing identity/state fails closed.
        let shared_harvest_rejected = if shared_harvest_direction.is_some() {
            match self.current_zone_session_id() {
                Some(session_id) => {
                    let synced = self
                        .sync_authoritative_zone_combat_state(&session_id)
                        .map(|sync_packets| {
                            packets.extend(sync_packets);
                        })
                        .is_some();
                    !synced
                        || !self
                            .zone_state
                            .lock()
                            .expect("shared zone presence mutex should not be poisoned")
                            .zone_manager
                            .player_harvest_admitted(&session_id, Self::zone_now_ms())
                }
                None => true,
            }
        } else {
            false
        };
        unavailable_shared_target |= shared_harvest_rejected;
        if is_world_tick
            && self.recent_zone_player_movement_input_window_active(Self::zone_now_ms())
        {
            self.filter_stale_owner_dead_entity_packets(&mut packets);
            return Ok(packets);
        }
        if !unavailable_shared_target {
            if let Some(object_id) = shared_action_target_id {
                self.apply_shared_action_target_snapshot(object_id);
            } else if let Some(direction) = shared_harvest_direction {
                // Harvest only needs the authoritative corpse in front of the
                // player. Reconciling every shared monster advances the
                // personal compatibility runtime once per entity; on a dense
                // map that can run dozens of unrelated AI ticks (and even kill
                // the private player) before the Harvest command executes.
                self.apply_shared_harvest_target_to_local(direction);
            } else if needs_shared_action_snapshot {
                self.apply_shared_current_map_monsters_to_local();
            }
        }
        if syncs_shared_npc_world_state {
            self.apply_shared_npc_saved_values_to_local();
            self.apply_shared_npc_random_seed_to_local();
        }
        // Snapshot reconciliation can advance the private runtime while
        // materializing a shared target. Re-assert the Zone transform at the
        // actual command boundary as well as before the shared-action gate.
        if syncs_inner_position_before_session_execute {
            self.force_inner_to_current_zone_transform();
        }
        let shared_npc_entity_baseline = if syncs_shared_npc_world_state {
            Some(self.inner.world_snapshot())
        } else {
            None
        };
        let blocks_durable_trade_mutation =
            is_trade_state_mutation && self.has_pending_durable_trade_projection();
        let mut command_packets = if blocks_durable_trade_mutation {
            Vec::new()
        } else if unavailable_shared_target {
            if shared_harvest_direction.is_some() {
                self.authoritative_zone_owner_correction()
            } else {
                Vec::new()
            }
        } else if let Some(locked) = shared_trade_confirm {
            self.execute_shared_trade_confirm(locked)
        } else if is_trade_cancel {
            if self.has_pending_durable_trade_projection() {
                Vec::new()
            } else {
                let cancel_packets = self.cancel_pending_shared_trade_offers();
                if cancel_packets.is_empty() {
                    self.inner.shared_trade_cancel(false)
                } else {
                    cancel_packets
                }
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
            if self.inner.has_active_shared_trade_state()
                || self.has_pending_durable_trade_projection()
            {
                Vec::new()
            } else {
                self.inner.trade_request(&partner_name)
            }
        } else if let Some(amount) = shared_gold_drop_amount {
            self.execute_shared_gold_drop(amount)
        } else if let Some((unique_id, count, hero_inventory)) = shared_inventory_item_drop {
            match self.execute_shared_inventory_item_drop(unique_id, count, hero_inventory) {
                Some(packets) => packets,
                None => self.inner.execute(command)?,
            }
        } else if let Some(object_id) = shared_pickup_object_id {
            match self.pick_up_shared_drop(object_id) {
                Some(shared_packets) => shared_packets,
                None => self.inner.execute(command)?,
            }
        } else if let Some((mouse_mode, location)) = shared_intelligent_creature_pickup {
            // Shared creature pickup is authoritative even when it produces no
            // packets. Never fall back to the personal SimulationSession path.
            self.pick_up_shared_drop_with_intelligent_creature(location, mouse_mode)
        } else if shared_harvest_direction.is_some() {
            self.inner.execute(command)?
        } else if let Some(attack) = zone_native_player_attack {
            self.execute_zone_native_player_attack(attack)
        } else if is_world_tick {
            // Session is personal; Zone is world. Drain the shared Zone above,
            // then advance only personal compatibility timers here. Running
            // the full private world tick would create a second monster/hazard
            // authority that can move, kill, or damage the same objects and
            // player independently of the Zone.
            self.inner.tick_shared_zone_personal_state()
        } else if is_game_shop_buy {
            let execution = self.inner.execute_with_outcome(command)?;
            self.last_game_shop_purchase_outcome = execution.game_shop_purchase_outcome;
            execution.packets
        } else if let WorldCommand::ClientPacket(packet) = &command {
            if let Some(zone_packets) = self.execute_zone_player_packet(packet) {
                zone_packets
            } else {
                self.inner.execute(command)?
            }
        } else {
            self.inner.execute(command)?
        };
        if removes_presence {
            self.cancel_pending_shared_rental_offers();
            packets.extend(self.remove_presence());
        }
        if shared_gold_drop_amount.is_some() || shared_inventory_item_drop.is_some() {
            self.remap_player_ground_drop_packets(&mut command_packets);
        }
        if command_packets.is_empty() {
            if let Some(object_id) = shared_interact_object_id {
                command_packets = self.execute_shared_npc_interact(object_id);
            } else if let Some((object_id, key)) = shared_call_npc {
                command_packets = self.execute_shared_npc_call(object_id, &key);
            }
        }
        if is_world_tick
            && !command_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::IntelligentCreaturePickup { .. }))
        {
            command_packets.extend(self.auto_pick_up_shared_drop_with_intelligent_creature());
        }
        if is_low_latency_zone_packet {
            packets.extend(command_packets);
            self.filter_stale_owner_dead_entity_packets(&mut packets);
            return Ok(packets);
        }
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
        if is_world_tick {
            self.suppress_personal_tick_shared_monster_motion(&mut command_packets);
        }
        self.apply_shared_entity_packets_to_current_map(&command_packets);
        if let Some(current_key) = self.current_presence_key() {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .filter_stale_dead_entity_packets_for_key(&current_key, &mut command_packets);
        }
        self.dispatch_shared_entity_observer_packets(&command_packets);
        let committed_death_drop_packets =
            self.commit_shared_death_drops_to_current_map(&command_packets);
        command_packets.extend(committed_death_drop_packets);
        let shared_quest_packets = self.dispatch_shared_quest_share_packets(&command_packets);
        let owner_zone_state = self.local_self_object_id().and_then(|owner_id| {
            let packets = owner_zone_state_packets(owner_id, &command_packets);
            (!packets.is_empty()).then_some((owner_id, packets))
        });
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
            if let Some(owner_id) = self.local_self_object_id() {
                let mut delayed_packets = delayed_player_action_packets(owner_id, &command_packets);
                if let Some((_, state_packets)) = owner_zone_state.as_ref() {
                    for packet in state_packets {
                        if !delayed_packets.contains(packet) {
                            delayed_packets.push(packet.clone());
                        }
                    }
                }
                self.dispatch_zone_observer_packets(owner_id, &delayed_packets)
            } else {
                Vec::new()
            }
        } else if let Some((owner_id, state_packets)) = owner_zone_state {
            self.dispatch_zone_observer_packets(owner_id, &state_packets)
        } else {
            Vec::new()
        };
        packets.extend(command_packets);
        packets.extend(shared_quest_packets);
        packets.extend(observer_packets);
        let mut packets = packets;
        if is_transfer_map_command
            || is_authoritative_move_to
            || is_handoff_transform
            || applies_native_state
        {
            self.force_next_zone_transform_sync = true;
            self.owner_dead_entity_ids.clear();
        }
        if is_town_revive
            && packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::Revived))
        {
            self.force_next_zone_transform_sync = true;
        }
        if removes_presence {
            self.owner_dead_entity_ids.clear();
        }
        self.apply_shared_entity_packets_to_current_map(&packets);
        self.filter_stale_owner_dead_entity_packets(&mut packets);
        if !removes_presence && !skip_tail_zone_snapshot {
            packets.extend(self.sync_zone_snapshot());
        }
        if is_start_game && self.current_presence_key().is_some() {
            // The pre-command recovery pass cannot see an identity/presence
            // before StartGame. Resolve durable recovery immediately after the
            // authoritative Zone join so the first playable snapshot already
            // contains post-restart economy state.
            packets.extend(self.apply_pending_shared_trade_packets());
        }
        if let Some(current_key) = self.current_presence_key() {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .filter_stale_dead_entity_packets_for_key(&current_key, &mut packets);
        }
        self.filter_stale_owner_dead_entity_packets(&mut packets);
        Ok(packets)
    }

    fn execute_with_outcome(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let command_kind = command.kind();
        let skips_snapshot = matches!(
            &command,
            WorldCommand::ClientPacket(
                ClientPacket::KeepAlive { .. }
                    | ClientPacket::Turn { .. }
                    | ClientPacket::Walk { .. }
                    | ClientPacket::Run { .. }
            ) | WorldCommand::Tick
        );
        let packets = self.execute(command)?;
        let packet_count = packets.len();
        Ok(WorldCommandExecution {
            packets,
            outcome: WorldCommandOutcome {
                command_kind,
                packet_count,
                snapshot_tick: if skips_snapshot {
                    0
                } else {
                    self.world_snapshot().tick
                },
                active_identity: self.active_identity(),
            },
            game_shop_purchase_outcome: self.last_game_shop_purchase_outcome.take(),
        })
    }

    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        self.inner.supports_typed_game_shop_purchase_outcome()
    }

    fn world_snapshot(&self) -> WorldSnapshot {
        let mut snapshot = self.inner.world_snapshot();
        let personal_quest_icons = snapshot
            .entities
            .iter()
            .filter_map(|entity| entity.quest_icon.map(|icon| (entity.object_id, icon)))
            .collect::<BTreeMap<_, _>>();
        let current_key = self.current_presence_key();
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
            for entity in &mut snapshot.entities {
                entity.quest_icon = personal_quest_icons.get(&entity.object_id).copied();
            }
            snapshot.ground_drops = shared_map.ground_drops.into_values().collect();
        }
        if let (Some(key), Some(map_file_name)) =
            (current_key.as_ref(), snapshot.map_file_name.as_deref())
        {
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
            if let Some(session_id) = zone_state.zone_sessions.get(key) {
                if let Some((hp, max_hp, mp)) = zone_state.zone_manager.player_vitals(session_id) {
                    snapshot.player_hp = Some(hp);
                    snapshot.player_max_hp = Some(max_hp);
                    snapshot.player_mp = Some(mp);
                    if let Some(self_entity) = snapshot
                        .entities
                        .iter_mut()
                        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
                    {
                        self_entity.hp = Some(hp);
                        self_entity.max_hp = Some(max_hp);
                        self_entity.dead = hp <= 0;
                    }
                }
            }
        }
        let mut remote_players = zone_state
            .remote_player_entities(snapshot.map_file_name.as_deref(), current_key.as_ref());
        snapshot.entities.append(&mut remote_players);
        if let Some((self_x, self_y)) = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| (entity.x, entity.y))
        {
            let range = CRYSTAL_OBJECT_DATA_RANGE as u32;
            let visible =
                |x: i32, y: i32| self_x.abs_diff(x) <= range && self_y.abs_diff(y) <= range;
            snapshot
                .entities
                .retain(|entity| visible(entity.x, entity.y));
            snapshot.ground_drops.retain(|drop| visible(drop.x, drop.y));
        }
        snapshot.entities.sort_by_key(|entity| entity.object_id);
        snapshot
    }

    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.inner.active_identity()
    }

    fn active_character_checkpoint(&self) -> Option<CharacterSaveRecord> {
        self.inner.active_character_checkpoint()
    }

    fn restore_active_character_checkpoint(
        &mut self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.inner.restore_active_character_checkpoint(checkpoint)
    }

    fn save_active_character(&mut self) -> Result<(), String> {
        self.sync_pending_zone_movement_transform()?;
        self.inner.save_active_character()
    }

    fn refresh_active_external_mail(&mut self) -> bool {
        self.inner.refresh_active_external_mail()
    }
}

const GLOBAL_ZONE_MESSAGE_BACKLOG: usize = 256;

#[derive(Debug, Default)]
pub(crate) struct GlobalZoneMessageBus {
    endpoints: Mutex<BTreeMap<String, GlobalZoneMessageEndpoint>>,
    next_registration_id: AtomicU64,
}

#[derive(Debug)]
struct GlobalZoneMessageEndpoint {
    zone_id: ZoneId,
    active_identity: bool,
    pending: VecDeque<ServerPacket>,
    live: Option<GlobalZoneMessageLiveSender>,
}

#[derive(Debug)]
struct GlobalZoneMessageLiveSender {
    registration_id: u64,
    sender: SharedZoneLiveOutboundSender,
    active: bool,
}

impl GlobalZoneMessageBus {
    pub(crate) fn register_session(&self, session_id: &str, zone_id: ZoneId) {
        if let Ok(mut endpoints) = self.endpoints.lock() {
            endpoints
                .entry(session_id.to_string())
                .and_modify(|endpoint| endpoint.zone_id = zone_id.clone())
                .or_insert_with(|| GlobalZoneMessageEndpoint {
                    zone_id,
                    active_identity: false,
                    pending: VecDeque::new(),
                    live: None,
                });
        }
    }

    pub(crate) fn update_session(&self, session_id: &str, zone_id: ZoneId, active_identity: bool) {
        if let Ok(mut endpoints) = self.endpoints.lock() {
            let endpoint = endpoints.entry(session_id.to_string()).or_insert_with(|| {
                GlobalZoneMessageEndpoint {
                    zone_id: zone_id.clone(),
                    active_identity,
                    pending: VecDeque::new(),
                    live: None,
                }
            });
            endpoint.zone_id = zone_id;
            endpoint.active_identity = active_identity;
        }
    }

    pub(crate) fn unregister_session(&self, session_id: &str) {
        if let Ok(mut endpoints) = self.endpoints.lock() {
            endpoints.remove(session_id);
        }
    }

    pub(crate) fn register_live(
        self: &Arc<Self>,
        session_id: &str,
        registration_id: Option<u64>,
        sender: SharedZoneLiveOutboundSender,
    ) -> GlobalZoneMessageRegistration {
        let registration_id = registration_id.unwrap_or_else(|| {
            self.next_registration_id
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1)
                .max(1)
        });
        if let Ok(mut endpoints) = self.endpoints.lock() {
            if let Some(endpoint) = endpoints.get_mut(session_id) {
                endpoint.live = Some(GlobalZoneMessageLiveSender {
                    registration_id,
                    sender,
                    active: false,
                });
            }
        }
        GlobalZoneMessageRegistration {
            bus: Arc::clone(self),
            session_id: session_id.to_string(),
            registration_id,
        }
    }

    fn activate_live(&self, session_id: &str, registration_id: u64) {
        let Ok(mut endpoints) = self.endpoints.lock() else {
            return;
        };
        let Some(endpoint) = endpoints.get_mut(session_id) else {
            return;
        };
        let Some(live) = endpoint.live.as_mut() else {
            return;
        };
        if live.registration_id != registration_id {
            return;
        }
        live.active = true;
        while let Some(packet) = endpoint.pending.pop_front() {
            match live
                .sender
                .try_send(SharedZoneLiveOutbound::new(registration_id, packet))
            {
                Ok(()) => {}
                Err(TokioTrySendError::Full(outbound)) => {
                    endpoint.pending.push_front(outbound.into_packet());
                    break;
                }
                Err(TokioTrySendError::Closed(outbound)) => {
                    endpoint.pending.push_front(outbound.into_packet());
                    endpoint.live = None;
                    break;
                }
            }
        }
    }

    fn unregister_live(&self, session_id: &str, registration_id: u64) {
        if let Ok(mut endpoints) = self.endpoints.lock() {
            if let Some(endpoint) = endpoints.get_mut(session_id) {
                if endpoint
                    .live
                    .as_ref()
                    .is_some_and(|live| live.registration_id == registration_id)
                {
                    endpoint.live = None;
                }
            }
        }
    }

    pub(crate) fn publish_to_other_zones(
        &self,
        source_session_id: &str,
        source_zone_id: &ZoneId,
        packets: &[ServerPacket],
    ) {
        if packets.is_empty() {
            return;
        }
        let Ok(mut endpoints) = self.endpoints.lock() else {
            return;
        };
        for (session_id, endpoint) in endpoints.iter_mut() {
            if session_id == source_session_id
                || &endpoint.zone_id == source_zone_id
                || !endpoint.active_identity
            {
                continue;
            }
            for packet in packets.iter().cloned() {
                let packet = match endpoint.live.as_mut() {
                    Some(live) if live.active => match live
                        .sender
                        .try_send(SharedZoneLiveOutbound::new(live.registration_id, packet))
                    {
                        Ok(()) => continue,
                        Err(TokioTrySendError::Full(outbound)) => outbound.into_packet(),
                        Err(TokioTrySendError::Closed(outbound)) => {
                            let packet = outbound.into_packet();
                            endpoint.live = None;
                            packet
                        }
                    },
                    _ => packet,
                };
                if endpoint.pending.len() >= GLOBAL_ZONE_MESSAGE_BACKLOG {
                    endpoint.pending.pop_front();
                }
                endpoint.pending.push_back(packet);
            }
        }
    }

    pub(crate) fn drain(&self, session_id: &str) -> Vec<ServerPacket> {
        self.endpoints
            .lock()
            .ok()
            .and_then(|mut endpoints| {
                endpoints
                    .get_mut(session_id)
                    .map(|endpoint| endpoint.pending.drain(..).collect())
            })
            .unwrap_or_default()
    }
}

pub(crate) struct GlobalZoneMessageRegistration {
    bus: Arc<GlobalZoneMessageBus>,
    session_id: String,
    registration_id: u64,
}

impl GlobalZoneMessageRegistration {
    pub(crate) fn registration_id(&self) -> u64 {
        self.registration_id
    }

    pub(crate) fn activate(&self) {
        self.bus
            .activate_live(&self.session_id, self.registration_id);
    }
}

impl fmt::Debug for GlobalZoneMessageRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlobalZoneMessageRegistration")
            .field("session_id", &self.session_id)
            .field("registration_id", &self.registration_id)
            .finish_non_exhaustive()
    }
}

impl Drop for GlobalZoneMessageRegistration {
    fn drop(&mut self) {
        self.bus
            .unregister_live(&self.session_id, self.registration_id);
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
    global_message_bus: Arc<GlobalZoneMessageBus>,
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
            global_message_bus: Arc::new(GlobalZoneMessageBus::default()),
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
        self.try_open_session_for(config, route_request)
            .expect("Zone session routing should succeed")
    }

    pub fn try_open_session_for(
        &self,
        config: GatewayConfig,
        route_request: SessionRouteRequest,
    ) -> Result<RoutedZoneRuntime, String> {
        let zone_id = self.try_route_session(&route_request)?;
        let owner_lease = self.owner_lease_authority.owner_lease(&zone_id);
        Ok(RoutedZoneRuntime {
            runtime: self.runtime_factory.create_runtime(config, &zone_id),
            owner_lease,
            owner_lease_authority: self.owner_lease_authority.clone(),
            zone_id,
        })
    }

    pub fn route_session(&self, route_request: &SessionRouteRequest) -> ZoneId {
        self.session_router
            .route_session(route_request, &self.default_zone_id)
    }

    pub fn try_route_session(&self, route_request: &SessionRouteRequest) -> Result<ZoneId, String> {
        self.session_router
            .try_route_session(route_request, &self.default_zone_id)
    }

    pub fn release_session(
        &self,
        route_request: &SessionRouteRequest,
        now_ms: u64,
    ) -> Result<(), String> {
        self.session_router.release_session(route_request, now_ms)
    }

    pub(crate) fn global_message_bus(&self) -> Arc<GlobalZoneMessageBus> {
        Arc::clone(&self.global_message_bus)
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
        delayed_player_action_packets, filter_stale_owner_dead_entity_packets,
        gateway_zone_magic_requires_item_consumption, gateway_zone_magic_targets_ground,
        gateway_zone_magic_targets_summon, ground_drop_spawn_packet,
        reconcile_shared_entity_with_native_monster, shared_entity_observer_packet_object_id,
        shared_gateway_now_ms, shared_npc_entity_side_effect_packets, shared_zone_movement_ingress,
        suppress_personal_tick_shared_monster_motion, sync_zone_movement_transform,
        world_entity_from_monster_info, world_entity_from_object_player_info,
        zone_monster_spawn_from_shared_entity, HostedZoneOwnerCommandClient,
        InMemoryZoneOwnerLeaseAuthority, InProcessAccountInventoryService,
        InProcessNpcWorldService, InProcessZoneRuntimeFactory, MapZoneSessionRouter,
        PerMapSessionRouter, SessionRouteRequest, SessionRouter, SharedAccountInventoryCommand,
        SharedAccountInventoryCommandEnvelope, SharedAccountInventoryCommitOutcome,
        SharedAccountInventoryExecutionContext, SharedAccountInventoryService,
        SharedAccountInventoryServiceHandle, SharedDropPickupResult,
        SharedInProcessZoneFactoryCheckpoint, SharedInProcessZoneRuntimeFactory,
        SharedInProcessZoneSessionRuntime, SharedInProcessZoneState, SharedNpcEntitySideEffect,
        SharedNpcWorldCommand, SharedNpcWorldCommandEnvelope, SharedNpcWorldService,
        SharedNpcWorldServiceHandle, SharedNpcWorldTransactionReceipt, SharedSessionRouter,
        SharedTradeSettlementOutcome, SharedZoneMovementIngress, SharedZoneMutationGate,
        SharedZoneRuntimeFactory, TradeProjectionReconciliationState,
        UnresolvedSharedTradeSettlement, ZoneId, ZoneMapSnapshotLayer, ZoneNativePlayerAttack,
        ZoneNativePlayerAttackKind, ZoneOwnerCommandRequest, ZoneOwnerLease, ZonePresenceKey,
        ZoneRegistry, ZoneRuntimeFactory,
    };

    use crate::{GatewayConfig, GatewaySession};
    use mir2_protocol::{
        ClientBuff, ClientIntelligentCreature, ClientPacket, IntelligentCreatureItemFilter,
        IntelligentCreatureRules, MirClass, MirDirection, MirGender, MirGridType, MonsterInfo,
        NpcInfo, ObjectDiedInfo, ObjectHealthInfo, ObjectItemInfo, ObjectMovement,
        ObjectPlayerInfo, ObjectRevivedInfo, ObjectStruckInfo, Point, ServerPacket, Spell,
        UserItemStat,
    };
    use mir2_simulation::{
        GroundDropClaimTicket, GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime,
        QuestStage, SessionId, SharedAccountInventoryTransactionKind,
        SharedAccountInventoryTransactionReceipt, SharedNpcSavedValue, SharedTradeOffer,
        WorldCommand, WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot, WorldRuntime,
        ZoneBossRewardAudit, ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
        ZoneMapMetadata, ZoneMonsterDefense, ZoneMonsterKillAward, ZoneMonsterSpawn,
        ZoneNativeMonsterSnapshot, ZoneNpcTeleportConfig, ZoneNpcTeleportDestination, ZoneOutbound,
        ZonePlayerCombatStats, ZoneRuntime, ZoneRuntimeHandle,
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{
            mpsc::{channel, sync_channel},
            Arc, Barrier, Condvar, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn atomic_world_checkpoint_restore_preserves_factory_when_later_zone_is_invalid() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let original_zone = ZoneId::new("atomic-original");
        let original_resources = factory.resources_for_zone(&original_zone);
        original_resources
            .zone_state
            .lock()
            .unwrap()
            .next_zone_object_id = 91_337;
        let original_bytes = factory.world_checkpoint_bytes().unwrap();

        let mut candidate: SharedInProcessZoneFactoryCheckpoint =
            serde_json::from_slice(&original_bytes).unwrap();
        let mut corrupt_later_zone = candidate
            .zones
            .values()
            .next()
            .expect("the original Zone checkpoint must exist")
            .clone();
        corrupt_later_zone.version = corrupt_later_zone.version.saturating_add(1);
        candidate
            .zones
            .insert(ZoneId::new("zz-corrupt-later-zone"), corrupt_later_zone);
        let candidate_bytes = serde_json::to_vec(&candidate).unwrap();

        let error = factory
            .install_world_checkpoint_bytes_atomically(&candidate_bytes)
            .expect_err("an invalid later Zone must reject the entire factory restore");
        assert!(error.contains("unsupported shared Zone state checkpoint version"));
        assert_eq!(factory.world_checkpoint_bytes().unwrap(), original_bytes);
        assert_eq!(factory.active_zone_count(), 1);
    }

    #[test]
    fn atomic_world_checkpoint_restore_preserves_replica_state_and_tick_flags() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let replica_zone = ZoneId::new("atomic-replica");
        factory.resources_for_zone(&replica_zone);
        factory.mark_zone_as_replica(&replica_zone);
        assert!(factory.is_zone_replica(&replica_zone));
        assert!(!factory.autonomous_ticks_enabled(&replica_zone));

        let mut candidate: SharedInProcessZoneFactoryCheckpoint =
            serde_json::from_slice(&factory.world_checkpoint_bytes().unwrap()).unwrap();
        let normal_zone = ZoneId::new("atomic-normal");
        let normal_checkpoint = candidate
            .zones
            .get(&replica_zone)
            .expect("the replica checkpoint must exist")
            .clone();
        candidate
            .zones
            .insert(normal_zone.clone(), normal_checkpoint);

        let restored = factory
            .install_world_checkpoint_bytes_atomically(&serde_json::to_vec(&candidate).unwrap())
            .unwrap();
        assert_eq!(restored, 2);
        assert_eq!(factory.active_zone_count(), 2);
        assert!(factory.is_zone_replica(&replica_zone));
        assert!(!factory.autonomous_ticks_enabled(&replica_zone));
        assert!(!factory.is_zone_replica(&normal_zone));
        assert!(factory.autonomous_ticks_enabled(&normal_zone));

        factory.promote_zone_from_replica(&replica_zone).unwrap();
        assert!(!factory.is_zone_replica(&replica_zone));
        assert!(factory.autonomous_ticks_enabled(&replica_zone));
    }

    #[test]
    fn failed_replica_promotion_preserves_the_replica_marker() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let missing_zone = ZoneId::new("missing-replica-state");
        factory.mark_zone_as_replica(&missing_zone);

        let error = factory
            .promote_zone_from_replica(&missing_zone)
            .expect_err("a replica without installed state must not be promoted");
        assert!(error.contains("has no installed replica state"));
        assert!(factory.is_zone_replica(&missing_zone));
    }
    #[test]
    fn first_resource_creation_honors_an_existing_replica_marker() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let replica_zone = ZoneId::new("replica-before-resource");
        factory.mark_zone_as_replica(&replica_zone);

        factory.resources_for_zone(&replica_zone);

        assert!(factory.is_zone_replica(&replica_zone));
        assert!(!factory.autonomous_ticks_enabled(&replica_zone));
    }

    #[test]
    fn economy_source_sequence_survives_checkpoint_and_exhaustion_fails_closed() {
        let mut state = SharedInProcessZoneState::new();
        assert_eq!(state.next_economy_source_sequence(), Some(1));
        assert_eq!(state.next_economy_source_sequence(), Some(2));

        let checkpoint = state.checkpoint().expect("checkpoint");
        let mut restored = SharedInProcessZoneState::restore(checkpoint).expect("restore");
        assert_eq!(restored.next_economy_source_sequence(), Some(3));

        restored.next_economy_source_sequence = u64::MAX - 1;
        assert_eq!(restored.next_economy_source_sequence(), Some(u64::MAX));
        assert_eq!(restored.next_economy_source_sequence(), None);

        let exhausted_checkpoint = restored.checkpoint().expect("exhausted checkpoint");
        let mut exhausted =
            SharedInProcessZoneState::restore(exhausted_checkpoint).expect("exhausted restore");
        assert_eq!(exhausted.next_economy_source_sequence(), None);
        assert_eq!(exhausted.next_economy_source_sequence, u64::MAX);
    }

    #[test]
    fn pending_zone_packets_never_exceed_rpc_outbound_capacity() {
        let mut state = SharedInProcessZoneState::new();
        let key = ZonePresenceKey {
            account_id: "stale-session".to_string(),
            character_index: 0,
        };

        for index in 0..=crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES {
            state.queue_zone_packets(
                key.clone(),
                vec![ServerPacket::Chat {
                    message: format!("packet-{index}"),
                    chat_type: mir2_protocol::ChatType::Normal,
                }],
            );
        }

        let pending = state
            .pending_zone_packets
            .get(&key)
            .expect("the bounded pending queue should remain present");
        assert_eq!(
            pending.len(),
            crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES
        );
        assert!(matches!(
            pending.first(),
            Some(ServerPacket::Chat { message, .. }) if message == "packet-1"
        ));
    }

    #[test]
    fn legacy_checkpoint_pending_packets_are_truncated_on_restore() {
        let state = SharedInProcessZoneState::new();
        let key = ZonePresenceKey {
            account_id: "legacy-orphan".to_string(),
            character_index: 0,
        };
        let mut checkpoint = state.checkpoint().expect("checkpoint");
        checkpoint.pending_zone_packet_frames = vec![(
            key.clone(),
            (0..=crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES)
                .map(|index| {
                    mir2_protocol::encode_server_packet(&ServerPacket::Chat {
                        message: format!("legacy-{index}"),
                        chat_type: mir2_protocol::ChatType::Normal,
                    })
                    .expect("packet frame")
                })
                .collect(),
        )];

        let restored = SharedInProcessZoneState::restore(checkpoint).expect("legacy restore");
        let pending = &restored.pending_zone_packets[&key];
        assert_eq!(
            pending.len(),
            crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_OUTBOUND_MESSAGES
        );
        assert!(matches!(
            pending.first(),
            Some(ServerPacket::Chat { message, .. }) if message == "legacy-1"
        ));
    }

    #[test]
    fn checkpoint_restore_requires_pending_drop_claims_to_match_presence_and_zone() {
        let mut state = SharedInProcessZoneState::new();
        let key = ZonePresenceKey {
            account_id: "claim-restore-account".to_string(),
            character_index: 0,
        };
        let session_id = SessionId::new("claim-restore-session");
        let zone_object_id = state.upsert_player(
            key.clone(),
            "ClaimRestorer",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        state.zone_sessions.insert(key.clone(), session_id.clone());
        state
            .zone_session_keys
            .insert(session_id.clone(), key.clone());
        state.zone_manager.join(ZoneJoin {
            session_id: session_id.clone(),
            account_id: key.account_id.clone(),
            character_index: key.character_index,
            object_id: zone_object_id,
            name: "ClaimRestorer".to_string(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 1,
            hp: 10,
            max_hp: 10,
            mp: 10,
            map_file_name: "0".to_string(),
            position: Point { x: 330, y: 270 },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        });
        let drop = GroundDropSnapshot {
            object_id: 8_801,
            name: "Checkpoint Claim Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: 330,
            y: 270,
            quantity: 1,
            source_monster: "checkpoint-claim-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 25 },
        };
        state.zone_manager.handle(ZoneCommand::SyncGroundDrops {
            session_id: session_id.clone(),
            drops: vec![drop.clone()],
            now_ms: 1_000,
        });
        let outbounds = state.zone_manager.handle(ZoneCommand::ClaimGroundDrop {
            session_id: session_id.clone(),
            object_id: Some(drop.object_id),
            target: Point { x: 330, y: 270 },
            group_members: Vec::new(),
            now_ms: 1_001,
        });
        let _ = state.dispatch_zone_outbounds(outbounds, None);
        assert_eq!(state.pending_zone_ground_drop_claims[&key].len(), 1);

        let checkpoint = state.checkpoint().expect("checkpoint with pending claim");
        SharedInProcessZoneState::restore(checkpoint.clone())
            .expect("an exact pending claim should restore");

        let expected_ticket = checkpoint.pending_zone_ground_drop_claims[0].1[0].clone();
        let mut missing_gateway_pending_entry = checkpoint.clone();
        missing_gateway_pending_entry
            .pending_zone_ground_drop_claims
            .clear();
        let restored = SharedInProcessZoneState::restore(missing_gateway_pending_entry)
            .expect("the authoritative Zone ticket should hydrate the missing Gateway entry");
        assert_eq!(
            restored.pending_zone_ground_drop_claims.get(&key),
            Some(&vec![expected_ticket])
        );

        let mut tampered_ticket = checkpoint.clone();
        tampered_ticket.pending_zone_ground_drop_claims[0].1[0]
            .idempotency_key
            .push_str("-tampered");
        let error = SharedInProcessZoneState::restore(tampered_ticket)
            .expect_err("a pending ticket absent from the restored Zone must fail");
        assert!(error.contains("absent or mismatched in Zone"));

        let mut wrong_session = checkpoint.clone();
        wrong_session.pending_zone_ground_drop_claims[0].1[0].session_id =
            SessionId::new("wrong-session");
        let error = SharedInProcessZoneState::restore(wrong_session)
            .expect_err("a pending ticket with the wrong presence session must fail");
        assert!(error.contains("session mismatch"));

        let mut orphaned_presence = checkpoint;
        orphaned_presence.pending_zone_ground_drop_claims[0]
            .0
            .account_id = "orphaned-claim-account".to_string();
        let error = SharedInProcessZoneState::restore(orphaned_presence)
            .expect_err("a pending ticket without a presence must fail");
        assert!(error.contains("without presence"));
    }
    #[test]
    fn world_checkpoint_removes_sessions_players_and_pending_packets() {
        let mut state = SharedInProcessZoneState::new();
        let key = ZonePresenceKey {
            account_id: "orphan-owner".to_string(),
            character_index: 0,
        };
        let session_id = SessionId::new("orphan-session");
        let zone_object_id = state.upsert_player(
            key.clone(),
            "Orphan",
            "0".to_string(),
            shared_picker_entity(101, 330, 270),
            80,
        );
        state.zone_sessions.insert(key.clone(), session_id.clone());
        state
            .zone_session_keys
            .insert(session_id.clone(), key.clone());
        state.zone_manager.join(ZoneJoin {
            session_id: session_id.clone(),
            account_id: key.account_id.clone(),
            character_index: key.character_index,
            object_id: zone_object_id,
            name: "Orphan".to_string(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 1,
            hp: 10,
            max_hp: 10,
            mp: 10,
            map_file_name: "0".to_string(),
            position: Point { x: 330, y: 270 },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        });

        let mut player = state
            .players
            .get(&key)
            .expect("player presence should exist")
            .entity
            .clone();
        player.object_id = zone_object_id;
        let mut owned_monster = shared_picker_entity(501, 331, 270);
        owned_monster.kind = WorldEntityKind::Monster;
        owned_monster.owner_name = Some("Orphan".to_string());
        let mut persistent_monster = shared_picker_entity(777, 332, 270);
        persistent_monster.kind = WorldEntityKind::Monster;
        let map = state.maps.entry("0".to_string()).or_default();
        map.entities.insert(player.object_id, player);
        map.entities.insert(owned_monster.object_id, owned_monster);
        map.entities
            .insert(persistent_monster.object_id, persistent_monster);
        state.queue_zone_packets(
            key,
            vec![ServerPacket::Chat {
                message: "never restore me".to_string(),
                chat_type: mir2_protocol::ChatType::Normal,
            }],
        );

        let checkpoint = state.world_checkpoint().expect("world checkpoint");
        assert!(checkpoint.zone_sessions.is_empty());
        assert!(checkpoint.zone_session_keys.is_empty());
        assert!(checkpoint.pending_zone_packet_frames.is_empty());
        assert!(checkpoint.players.is_empty());
        assert_eq!(
            checkpoint.maps["0"]
                .entities
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![777]
        );

        let restored = SharedInProcessZoneState::restore(checkpoint).expect("restore world image");
        assert!(restored.zone_sessions.is_empty());
        assert!(restored.pending_zone_packets.is_empty());
        assert!(restored.players.is_empty());
        assert!(restored
            .zone_manager
            .player_transform(&session_id)
            .is_none());
    }

    #[test]
    fn personal_tick_drops_shared_monster_motion_but_preserves_other_packets() {
        let mut packets = vec![
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 9_100,
                    position: Point { x: 10, y: 10 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::ObjectTurn {
                movement: ObjectMovement {
                    object_id: 101,
                    position: Point { x: 8, y: 8 },
                    direction: MirDirection::Left,
                },
            },
            ServerPacket::Chat {
                message: "keep me".to_string(),
                chat_type: mir2_protocol::ChatType::Normal,
            },
        ];

        suppress_personal_tick_shared_monster_motion(&mut packets, &BTreeSet::from([9_100]));

        assert_eq!(packets.len(), 2);
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectTurn { movement } if movement.object_id == 101
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Chat { message, .. } if message == "keep me"
        )));
    }

    #[test]
    fn shared_zone_mutation_gate_hands_off_to_exact_next_ticket() {
        let gate = Arc::new(SharedZoneMutationGate::default());
        let zone_id = ZoneId::new("gate-test");
        let first = gate
            .lock_zone(&zone_id)
            .expect("first ticket should acquire");
        let (acquired_sender, acquired_receiver) = channel();
        let mut workers = Vec::new();

        for worker_id in 0..32 {
            let worker_gate = Arc::clone(&gate);
            let worker_zone_id = zone_id.clone();
            let acquired_sender = acquired_sender.clone();
            workers.push(thread::spawn(move || {
                let _guard = worker_gate
                    .lock_zone(&worker_zone_id)
                    .expect("queued ticket should acquire");
                acquired_sender
                    .send(worker_id)
                    .expect("acquisition should be observed");
            }));

            let expected_next_ticket = u64::try_from(worker_id + 2).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let queued = gate
                    .state
                    .lock()
                    .expect("gate state should lock")
                    .lanes
                    .get(&zone_id)
                    .expect("test Zone lane should exist")
                    .next_ticket
                    == expected_next_ticket;
                if queued {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "worker {worker_id} did not enqueue its ticket"
                );
                thread::yield_now();
            }
        }

        drop(acquired_sender);
        drop(first);
        let acquired = acquired_receiver.iter().collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("queued worker should finish");
        }
        assert_eq!(acquired, (0..32).collect::<Vec<_>>());
    }

    #[test]
    fn shared_zone_mutation_gate_runs_zones_in_parallel_but_fences_host_checkpoint() {
        let gate = Arc::new(SharedZoneMutationGate::default());
        let first_zone = ZoneId::new("gate-a");
        let second_zone = ZoneId::new("gate-b");
        let first_guard = gate
            .lock_zone(&first_zone)
            .expect("first Zone should acquire");

        let (second_sender, second_receiver) = channel();
        let second_gate = Arc::clone(&gate);
        let second_worker = thread::spawn(move || {
            let _guard = second_gate
                .lock_zone(&second_zone)
                .expect("unrelated Zone should acquire");
            second_sender.send(()).expect("second Zone should report");
        });
        second_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("unrelated Zone must not wait behind first Zone");
        second_worker
            .join()
            .expect("second Zone worker should finish");

        let (checkpoint_sender, checkpoint_receiver) = channel();
        let checkpoint_gate = Arc::clone(&gate);
        let checkpoint_worker = thread::spawn(move || {
            let _guard = checkpoint_gate
                .lock()
                .expect("host checkpoint scope should acquire");
            checkpoint_sender
                .send(())
                .expect("checkpoint scope should report");
        });
        assert!(
            checkpoint_receiver
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "host checkpoint must wait for active Zone writers"
        );
        drop(first_guard);
        checkpoint_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("host checkpoint should acquire after Zone writers finish");
        checkpoint_worker
            .join()
            .expect("checkpoint worker should finish");
    }

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
                ..SessionRouteRequest::anonymous()
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
                    ..SessionRouteRequest::anonymous()
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
                    ..SessionRouteRequest::anonymous()
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
        GatewaySession::new_with_zone_registry_route(
            GatewayConfig::default(),
            registry,
            SessionRouteRequest {
                account_id: Some(account.to_string()),
                character_index: Some(0),
                map_file_name: Some(map.to_string()),
                ..SessionRouteRequest::anonymous()
            },
        )
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
    fn map_zone_transfer_atomically_rebinds_to_the_destination_zone() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_new_character(&mut session, "wanderer", "Wanderer");
        assert_eq!(session.zone_id(), &ZoneId::new("map:0"));
        assert_eq!(session.handoff_generation(), 1);

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
            &ZoneId::new("map:0102"),
            "the committed map and bound Zone must move together"
        );
        assert_eq!(session.handoff_generation(), 2);
    }

    #[test]
    fn checkpoint_handoff_repairs_a_stale_target_projection_before_commit() {
        #[derive(Debug)]
        struct StaleStartGameRuntime {
            inner: InProcessWorldRuntime,
        }

        impl WorldRuntime for StaleStartGameRuntime {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn on_connect(&self) -> Vec<ServerPacket> {
                self.inner.on_connect()
            }

            fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
                let corrupt_after_start = matches!(
                    &command,
                    WorldCommand::ClientPacket(ClientPacket::StartGame { .. })
                );
                let packets = self.inner.execute(command)?;
                if corrupt_after_start {
                    let mut stale = self
                        .inner
                        .active_character_checkpoint()
                        .expect("StartGame should produce an active checkpoint");
                    stale.hp = stale.hp.saturating_sub(7);
                    stale.belt_items_json.clear();
                    self.inner
                        .restore_active_character_checkpoint(&stale)
                        .expect("test fixture should install stale target state");
                }
                Ok(packets)
            }

            fn world_snapshot(&self) -> mir2_simulation::WorldSnapshot {
                self.inner.world_snapshot()
            }

            fn active_identity(&self) -> Option<mir2_simulation::ActiveSessionIdentity> {
                self.inner.active_identity()
            }

            fn active_character_checkpoint(&self) -> Option<mir2_simulation::CharacterSaveRecord> {
                self.inner.active_character_checkpoint()
            }

            fn restore_active_character_checkpoint(
                &mut self,
                checkpoint: &mir2_simulation::CharacterSaveRecord,
            ) -> Result<(), String> {
                self.inner.restore_active_character_checkpoint(checkpoint)
            }

            fn save_active_character(&mut self) -> Result<(), String> {
                self.inner.save_active_character()
            }

            fn refresh_active_external_mail(&mut self) -> bool {
                self.inner.refresh_active_external_mail()
            }
        }

        #[derive(Debug)]
        struct StaleMapOneFactory {
            inner: SharedInProcessZoneRuntimeFactory,
        }

        impl ZoneRuntimeFactory for StaleMapOneFactory {
            fn create_runtime(&self, config: GatewayConfig, zone_id: &ZoneId) -> ZoneRuntimeHandle {
                if zone_id == &ZoneId::new("map:1") {
                    return Box::new(StaleStartGameRuntime {
                        inner: InProcessWorldRuntime::new(config),
                    });
                }
                self.inner.create_runtime(config, zone_id)
            }
        }

        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(StaleMapOneFactory {
                inner: SharedInProcessZoneRuntimeFactory::new(),
            }) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_new_character(&mut session, "handoff-checkpoint", "Checkpoint");
        let before = session.world_snapshot();
        let expected_hp = before.player_hp;
        let expected_belt_items = before.belt_items;

        session
            .execute_with_outcome(WorldCommand::TransferMap {
                key: "crystal:1:100:100".to_string(),
            })
            .expect("source checkpoint should repair stale target state");

        assert_eq!(session.zone_id(), &ZoneId::new("map:1"));
        assert_eq!(session.handoff_generation(), 2);
        let after = session.world_snapshot();
        assert_eq!(after.map_file_name.as_deref(), Some("1"));
        assert_eq!(after.player_hp, expected_hp);
        assert_eq!(after.belt_items, expected_belt_items);
    }

    #[test]
    fn failed_target_prepare_rolls_the_source_back_without_rebinding() {
        #[derive(Debug)]
        struct RejectPasskeyRuntime {
            inner: InProcessWorldRuntime,
        }

        impl WorldRuntime for RejectPasskeyRuntime {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn on_connect(&self) -> Vec<ServerPacket> {
                self.inner.on_connect()
            }

            fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
                if matches!(&command, WorldCommand::PasskeyLogin { .. }) {
                    return Err("injected target prepare failure".to_string());
                }
                self.inner.execute(command)
            }

            fn world_snapshot(&self) -> mir2_simulation::WorldSnapshot {
                self.inner.world_snapshot()
            }

            fn active_identity(&self) -> Option<mir2_simulation::ActiveSessionIdentity> {
                self.inner.active_identity()
            }

            fn save_active_character(&mut self) -> Result<(), String> {
                self.inner.save_active_character()
            }

            fn refresh_active_external_mail(&mut self) -> bool {
                self.inner.refresh_active_external_mail()
            }
        }

        #[derive(Debug)]
        struct FailMapOneFactory {
            inner: SharedInProcessZoneRuntimeFactory,
        }

        impl ZoneRuntimeFactory for FailMapOneFactory {
            fn create_runtime(&self, config: GatewayConfig, zone_id: &ZoneId) -> ZoneRuntimeHandle {
                if zone_id == &ZoneId::new("map:1") {
                    return Box::new(RejectPasskeyRuntime {
                        inner: InProcessWorldRuntime::new(config),
                    });
                }
                self.inner.create_runtime(config, zone_id)
            }
        }

        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(FailMapOneFactory {
                inner: SharedInProcessZoneRuntimeFactory::new(),
            }) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_new_character(&mut session, "handoff-rollback", "Rollback");
        assert_eq!(session.zone_id(), &ZoneId::new("map:0"));

        let error = session
            .execute_with_outcome(WorldCommand::TransferMap {
                key: "crystal:1:100:100".to_string(),
            })
            .expect_err("injected target prepare failure must abort handoff");

        assert!(error.contains("injected target prepare failure"));
        assert_eq!(session.zone_id(), &ZoneId::new("map:0"));
        assert_eq!(session.handoff_generation(), 1);
        assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
    }

    #[test]
    fn server_shout_crosses_zone_boundaries_through_the_global_bus() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        );
        let mut speaker =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        let mut listener =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_new_character(&mut speaker, "global-speaker", "Speaker");
        start_new_character(&mut listener, "global-listener", "Listener");
        listener.transfer_map("crystal:1:100:100");

        assert_eq!(speaker.zone_id(), &ZoneId::new("map:0"));
        assert_eq!(listener.zone_id(), &ZoneId::new("map:1"));
        let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
        let registration = listener
            .register_zone_live_outbound(sender)
            .expect("listener live registration should succeed")
            .expect("active listener should register");
        registration.activate();
        let live_packets = vec![ServerPacket::Chat {
            message: "(!)Speaker:live cross-zone".to_string(),
            chat_type: mir2_protocol::ChatType::Shout3,
        }];
        registry.global_message_bus().publish_to_other_zones(
            speaker.session_id(),
            speaker.zone_id(),
            &live_packets,
        );
        let live = receiver
            .try_recv()
            .expect("cross-Zone message should use the active socket channel");
        assert_eq!(live.registration_id(), registration.registration_id());
        assert!(matches!(
            live.into_packet(),
            ServerPacket::Chat { message, .. } if message.contains("live cross-zone")
        ));
        drop(registration);

        let owner_packets = vec![ServerPacket::Chat {
            message: "(!)Speaker:cross-zone hello".to_string(),
            chat_type: mir2_protocol::ChatType::Shout3,
        }];
        registry.global_message_bus().publish_to_other_zones(
            speaker.session_id(),
            speaker.zone_id(),
            &owner_packets,
        );
        let is_cross_zone_shout = |packet: &ServerPacket| match packet {
            ServerPacket::Chat { message, chat_type }
                if matches!(
                    chat_type,
                    mir2_protocol::ChatType::Shout
                        | mir2_protocol::ChatType::Shout2
                        | mir2_protocol::ChatType::Shout3
                ) =>
            {
                message.contains("cross-zone hello")
            }
            ServerPacket::ObjectChat {
                text, chat_type, ..
            } if matches!(
                chat_type,
                mir2_protocol::ChatType::Shout
                    | mir2_protocol::ChatType::Shout2
                    | mir2_protocol::ChatType::Shout3
            ) =>
            {
                text.contains("cross-zone hello")
            }
            _ => false,
        };
        assert!(owner_packets.iter().any(is_cross_zone_shout));

        let remote_packets = listener.handle_packet(ClientPacket::KeepAlive { time: 55 });
        assert!(remote_packets.iter().any(is_cross_zone_shout));
    }

    #[test]
    fn shared_in_process_factory_isolates_state_by_zone_id() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let mut bichon = factory.create_runtime(GatewayConfig::default(), &ZoneId::new("bichon-0"));
        let mut primary = factory.create_runtime(GatewayConfig::default(), &ZoneId::primary());

        start_new_runtime_handle(&mut bichon, "bichon-isolation", "Scout");
        start_new_runtime_handle(&mut primary, "primary-isolation", "Blade");

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
    fn observer_aoi_remove_does_not_tombstone_retained_native_monster_globally() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "aoi-remove", "Observer");
        let identity = runtime
            .inner
            .active_identity()
            .expect("started session should expose an identity");
        let key = ZonePresenceKey::from_identity(&identity);

        let mut state = zone_state.lock().expect("shared zone state should lock");
        let native = state
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map("0"))
            .into_iter()
            .find(|monster| !monster.dead && monster.hp > 0)
            .expect("started shared Zone should retain a native monster");
        assert!(state
            .map_layer(Some("0"))
            .and_then(|map| map.entities.get(&native.object_id).cloned())
            .is_some());

        state.apply_zone_packets_to_map_layer(
            &key,
            &[ServerPacket::ObjectRemove {
                object_id: native.object_id,
            }],
        );

        let projected = state
            .map_layer(Some("0"))
            .and_then(|map| map.entities.get(&native.object_id).cloned())
            .expect("observer-local remove must not delete the zone-wide action target");
        assert_eq!(projected.hp, Some(native.hp));
        assert!(!projected.dead);
        assert!(state.shared_entity_allows_action("0", native.object_id));
    }

    #[test]
    fn command_tail_aoi_remove_does_not_tombstone_retained_shared_npc_globally() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-command-tail-aoi-remove", "Observer");
        let npc = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == "Assistant_Jane")
            .expect("full Crystal runtime should expose Assistant Jane");

        assert!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .zone_manager
                .zone(&ZoneKey::for_map("0"))
                .is_some_and(|zone| zone.retains_object_id(npc.object_id)),
            "shared NPC should remain authoritative while outside one observer's AOI"
        );

        runtime.apply_shared_entity_packets_to_current_map(&[ServerPacket::ObjectRemove {
            object_id: npc.object_id,
        }]);

        let state = zone_state.lock().expect("shared zone state should lock");
        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should still exist");
        assert!(map.entities.contains_key(&npc.object_id));
        assert!(!map.removed_entity_ids.contains(&npc.object_id));
    }

    #[test]
    fn final_shared_entity_apply_does_not_tombstone_retained_shared_npc_globally() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-final-apply-aoi-remove", "Observer");
        let npc = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Npc)
            .expect("full Crystal runtime should expose a nearby shared NPC");

        let mut state = zone_state.lock().expect("shared zone state should lock");
        assert!(
            state
                .zone_manager
                .zone(&ZoneKey::for_map("0"))
                .is_some_and(|zone| zone.retains_object_id(npc.object_id)),
            "shared NPC should remain authoritative while outside one observer's AOI"
        );

        state.apply_shared_entity_packets(
            "0",
            &[ServerPacket::ObjectRemove {
                object_id: npc.object_id,
            }],
        );

        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should still exist");
        assert!(map.entities.contains_key(&npc.object_id));
        assert!(!map.removed_entity_ids.contains(&npc.object_id));
    }

    #[test]
    fn personal_viewport_remove_does_not_delete_retained_shared_npc_from_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-personal-viewport-remove", "Observer");
        let npc = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == "Assistant_Jane")
            .expect("full Crystal runtime should expose Assistant Jane");

        runtime.dispatch_shared_entity_observer_packets(&[ServerPacket::ObjectRemove {
            object_id: npc.object_id,
        }]);

        let state = zone_state.lock().expect("shared zone state should lock");
        let zone = state
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .expect("started runtime should retain its shared Zone");
        assert!(zone.retains_object_id(npc.object_id));
        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should still exist");
        assert!(map.entities.contains_key(&npc.object_id));
        assert!(!map.removed_entity_ids.contains(&npc.object_id));
    }

    #[test]
    fn player_observer_broadcast_does_not_delete_retained_shared_npc_from_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-player-observer-remove", "Observer");
        let npc = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == "Merchant_John")
            .expect("full Crystal runtime should expose Merchant John");
        let owner_local_object_id = runtime
            .local_self_object_id()
            .expect("started runtime should expose its local self object id");

        runtime.dispatch_zone_observer_packets(
            owner_local_object_id,
            &[ServerPacket::ObjectRemove {
                object_id: npc.object_id,
            }],
        );

        let state = zone_state.lock().expect("shared zone state should lock");
        let zone = state
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .expect("started runtime should retain its shared Zone");
        assert!(zone.retains_object_id(npc.object_id));
        let map = state
            .map_layer(Some("0"))
            .expect("shared map layer should still exist");
        assert!(map.entities.contains_key(&npc.object_id));
        assert!(!map.removed_entity_ids.contains(&npc.object_id));
    }

    #[test]
    fn shared_zone_seeds_current_map_npcs_for_later_owner_aoi_entry() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-aoi-seed", "Walker");
        let session_id = runtime
            .current_zone_session_id()
            .expect("started runtime should have a Zone session");

        let mut packets = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position: Point { x: 324, y: 262 },
                direction: MirDirection::UpLeft,
            },
            false,
        );
        packets.extend(runtime.sync_zone_snapshot());

        assert!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .zone_manager
                .zone(&ZoneKey::for_map("0"))
                .is_some_and(|zone| zone.retains_object_id(26)),
            "static NPCs outside the StartGame viewport must remain in the Zone for later AOI entry"
        );

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectNpc { info }
                if info.object_id == 26
                    && info.name == "MirGuide_Peter"
                    && info.location == (Point { x: 328, y: 258 })
        )));
    }

    #[test]
    fn retained_shared_npc_survives_owner_aoi_leave_and_reentry() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-aoi-round-trip", "Walker");
        let session_id = runtime
            .current_zone_session_id()
            .expect("started runtime should have a Zone session");
        let jane_object_id = 3;

        let away = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id: session_id.clone(),
                position: Point { x: 324, y: 262 },
                direction: MirDirection::UpLeft,
            },
            false,
        );
        assert!(away.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == jane_object_id
        )));
        runtime.apply_shared_entity_packets_to_current_map(&away);
        {
            let state = zone_state.lock().expect("shared zone state should lock");
            let map = state
                .map_layer(Some("0"))
                .expect("shared map layer should still exist");
            assert!(map.entities.contains_key(&jane_object_id));
            assert!(!map.removed_entity_ids.contains(&jane_object_id));
        }

        let returned = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position: Point { x: 284, y: 606 },
                direction: MirDirection::Down,
            },
            false,
        );
        assert!(returned.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectNpc { info }
                if info.object_id == jane_object_id && info.name == "Assistant_Jane"
        )));
        runtime.apply_shared_entity_packets_to_current_map(&returned);

        let snapshot = runtime.world_snapshot();
        assert!(snapshot.entities.iter().any(|entity| {
            entity.object_id == jane_object_id
                && entity.kind == WorldEntityKind::Npc
                && entity.name == "Assistant_Jane"
        }));
    }

    #[test]
    fn retained_shared_npc_survives_personal_ticks_outside_aoi() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "npc-personal-tick-aoi", "Walker");
        let session_id = runtime
            .current_zone_session_id()
            .expect("started runtime should have a Zone session");
        let jane_object_id = 3;

        let away = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id: session_id.clone(),
                position: Point { x: 290, y: 625 },
                direction: MirDirection::Down,
            },
            false,
        );
        runtime.apply_shared_entity_packets_to_current_map(&away);
        runtime.force_inner_to_current_zone_transform();
        for _ in 0..32 {
            runtime
                .execute(WorldCommand::Tick)
                .expect("personal world tick outside Jane AOI should execute");
        }
        {
            let state = zone_state.lock().expect("shared zone state should lock");
            assert!(
                state
                    .zone_manager
                    .zone(&ZoneKey::for_map("0"))
                    .is_some_and(|zone| zone.retains_object_id(jane_object_id)),
                "personal viewport ticks must not delete Jane from the Zone"
            );
            let map = state
                .map_layer(Some("0"))
                .expect("shared map layer should still exist");
            assert!(map.entities.contains_key(&jane_object_id));
            assert!(!map.removed_entity_ids.contains(&jane_object_id));
        }

        let returned = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position: Point { x: 284, y: 606 },
                direction: MirDirection::Down,
            },
            false,
        );
        runtime.apply_shared_entity_packets_to_current_map(&returned);
        runtime.force_inner_to_current_zone_transform();
        runtime
            .execute(WorldCommand::Tick)
            .expect("personal world tick after returning to Jane should execute");

        assert!(runtime.world_snapshot().entities.iter().any(|entity| {
            entity.object_id == jane_object_id
                && entity.kind == WorldEntityKind::Npc
                && entity.name == "Assistant_Jane"
        }));
    }

    #[test]
    fn shared_zone_seeds_distant_current_map_monsters_for_later_owner_aoi_entry() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner =
            InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
        start_new_runtime(&mut runtime, "monster-aoi-seed", "Walker");

        let forest_yeti_group = mir2_game_data::crystal_map_respawns_by_file_name("0")
            .expect("map 0 should have Crystal respawns")
            .respawns
            .into_iter()
            .find(|respawn| {
                respawn.monster_name == "ForestYeti"
                    && respawn.location == (Point { x: 110, y: 425 })
            })
            .expect("map 0 should have the q22 ForestYeti group");
        let target_position =
            mir2_simulation::crystal_world_respawn_spawns("0", &forest_yeti_group)
                .into_iter()
                .next()
                .map(|(_, position, _)| position)
                .expect("q22 ForestYeti group should contain a walkable spawn");
        let session_id = runtime
            .current_zone_session_id()
            .expect("started runtime should have a Zone session");

        let mut packets = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position: Point {
                    x: target_position.x.saturating_sub(4),
                    y: target_position.y.saturating_sub(4),
                },
                direction: MirDirection::DownRight,
            },
            false,
        );
        packets.extend(runtime.sync_newly_active_private_monsters_to_zone());
        let forest_yeti = runtime
            .inner
            .current_map_shared_entity_snapshots()
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.name == "ForestYeti"
                    && entity.x.abs_diff(target_position.x) <= 20
                    && entity.y.abs_diff(target_position.y) <= 20
            })
            .expect("moving toward q22 should activate a nearby ForestYeti in the private pool");

        assert!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .zone_manager
                .zone(&ZoneKey::for_map("0"))
                .is_some_and(|zone| zone.retains_object_id(forest_yeti.object_id)),
            "a newly activated distant monster must be promoted into the shared Zone"
        );

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.object_id == forest_yeti.object_id
                    && info.name == "ForestYeti"
        )));
    }

    #[test]
    fn shared_map_sync_preserves_zone_native_monster_authority() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "native-map-authority", "Authority");

        let (native, mut stale_private) = {
            let state = zone_state.lock().expect("shared zone state should lock");
            let native = state
                .zone_manager
                .native_monster_snapshots(&ZoneKey::for_map("0"))
                .into_iter()
                .find(|monster| !monster.dead && monster.hp > 1)
                .expect("started shared Zone should contain a live native monster");
            let stale_private = state
                .maps
                .get("0")
                .and_then(|map| map.entities.get(&native.object_id))
                .cloned()
                .expect("native monster should have a shared map projection");
            (native, stale_private)
        };

        stale_private.x = stale_private.x.saturating_add(37);
        stale_private.y = stale_private.y.saturating_add(29);
        stale_private.hp = Some(1);
        stale_private.dead = false;
        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            state.sync_map_layer(
                "0".to_string(),
                vec![stale_private],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
            let projected = state
                .map_layer(Some("0"))
                .and_then(|map| map.entities.get(&native.object_id).cloned())
                .expect("native monster projection should remain present");
            assert_eq!(
                (projected.x, projected.y),
                (native.position.x, native.position.y)
            );
            assert_eq!(projected.hp, Some(native.hp));
            assert_eq!(projected.max_hp, Some(native.max_hp));
            assert_eq!(projected.dead, native.dead);
            assert!(state.shared_entity_allows_action("0", native.object_id));
        }
    }

    #[test]
    fn zone_monster_spawn_from_shared_entity_restores_crystal_neutral_ai() {
        let mut guard = shared_monster_entity(9001);
        guard.name = "Royal_Guard".to_string();
        guard.ai = None;
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
        archer.ai = None;
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
    fn zone_monster_spawn_from_shared_entity_preserves_explicit_hostile_passive_override() {
        let mut deer = shared_monster_entity(9003);
        deer.name = "Deer".to_string();
        deer.disposition = WorldEntityDisposition::Hostile;
        deer.hp = None;
        deer.max_hp = None;
        deer.sprite = None;

        let deer_spawn = zone_monster_spawn_from_shared_entity(&deer, 0)
            .expect("Deer should be convertible to a native zone spawn");

        assert_eq!(
            deer_spawn.ai, 0,
            "GM/test hostile Deer must not be downgraded back to Crystal passive AI 2"
        );
        assert_eq!(deer_spawn.level, 12);
        assert_eq!(deer_spawn.max_hp, 25);
        assert_eq!(deer_spawn.experience, 18);
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
        monster.ai = None;
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
    fn world_entity_from_monster_info_fails_closed_without_explicit_disposition() {
        let mut guard_info = shared_monster_info(9003, 0);
        guard_info.name = "Royal_Guard".to_string();
        guard_info.ai = 6;

        let guard = world_entity_from_monster_info(&guard_info);
        assert_eq!(guard.disposition, WorldEntityDisposition::Neutral);

        let mut hostile_info = shared_monster_info(9004, 0);
        hostile_info.ai = 0;

        let unknown = world_entity_from_monster_info(&hostile_info);
        assert_eq!(unknown.disposition, WorldEntityDisposition::Neutral);
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
        // ObjectMonster has no authoritative relationship field, so the
        // shared-zone packet path remains fail-closed.
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
    fn finalized_world_event_monster_is_indexed_as_player_action_target() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let zone_id = ZoneId::primary();
        let spawn = ZoneMonsterSpawn {
            object_id: 0x7000_0042,
            name: "WoomaSoldier".to_string(),
            name_colour_argb: -1,
            image: 29,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 30,
            max_hp: 285,
            hp: 285,
            experience: 310,
            move_speed_ms: 0,
            attack_speed_ms: 0,
            friendly_guild: None,
            position: Point { x: 168, y: 155 },
            direction: MirDirection::Down,
            defense: ZoneMonsterDefense::default(),
            respawn: None,
            drops: Vec::new(),
        };

        assert_eq!(
            factory
                .apply_world_event_monsters(&zone_id, "D022", &[spawn.clone()], 1_000)
                .unwrap(),
            1
        );

        let resources = factory.resources_for_zone(&zone_id);
        let mut zone_state = resources.zone_state.lock().unwrap();
        zone_state.apply_shared_entity_packets(
            "D022",
            &[ServerPacket::ObjectMonster {
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
                    dead: false,
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
            }],
        );
        let entity = zone_state
            .maps
            .get("D022")
            .and_then(|map| map.entities.get(&spawn.object_id))
            .expect("world event monster must enter the shared player-action index");
        assert_eq!(entity.name, spawn.name);
        assert_eq!(entity.level, Some(spawn.level));
        assert_eq!(entity.hp, Some(spawn.hp));
        assert_eq!(entity.max_hp, Some(spawn.max_hp));
        assert_eq!(entity.disposition, WorldEntityDisposition::Hostile);
    }

    #[test]
    fn player_attack_routes_to_finalized_world_event_monster() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let zone_id = ZoneId::primary();
        let spawn = ZoneMonsterSpawn {
            object_id: 0x7000_0043,
            name: "WoomaSoldier".to_string(),
            name_colour_argb: -1,
            image: 29,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 30,
            max_hp: 285,
            hp: 285,
            experience: 310,
            move_speed_ms: 0,
            attack_speed_ms: 0,
            friendly_guild: None,
            position: Point { x: 168, y: 155 },
            direction: MirDirection::Down,
            defense: ZoneMonsterDefense::default(),
            respawn: None,
            drops: Vec::new(),
        };
        factory
            .apply_world_event_monsters(&zone_id, "D022", &[spawn.clone()], 1_000)
            .unwrap();
        let resources = factory.resources_for_zone(&zone_id);
        let mut runtime = shared_session_runtime(resources.zone_state);
        start_new_runtime(&mut runtime, "director-fighter", "DirectorFighter");
        runtime
            .execute(WorldCommand::TransferMap {
                key: "crystal:D022:168:156".to_string(),
            })
            .unwrap();
        let zone_session_id = runtime
            .current_zone_session_id()
            .expect("started runtime should have a Zone session id");
        {
            let mut zone_state = runtime.zone_state.lock().unwrap();
            let entity = zone_state
                .maps
                .get_mut("D022")
                .and_then(|map| map.entities.get_mut(&spawn.object_id))
                .expect("world event monster should be indexed");
            entity.x = 168;
            entity.y = 156;
            let _ = zone_state.dispatch_zone_outbounds(
                vec![ZoneOutbound::ToSession {
                    session_id: zone_session_id,
                    packets: vec![ServerPacket::ObjectWalk {
                        movement: ObjectMovement {
                            object_id: spawn.object_id,
                            position: spawn.position.clone(),
                            direction: MirDirection::Up,
                        },
                    }],
                }],
                None,
            );
            let entity = zone_state
                .maps
                .get("D022")
                .and_then(|map| map.entities.get(&spawn.object_id))
                .expect("world event monster should remain indexed");
            assert_eq!(
                (entity.x, entity.y),
                (spawn.position.x, spawn.position.y),
                "autonomous Zone packets must refresh the player-action index"
            );
        }

        let packets = runtime
            .execute(WorldCommand::Attack {
                object_id: spawn.object_id,
            })
            .unwrap();
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id != spawn.object_id
        )));
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
    fn object_player_info_preserves_authoritative_layered_appearance() {
        let mut info = shared_object_player_info(602, "RemoteArcher", 331, 271);
        info.class = MirClass::Archer;
        info.gender = MirGender::Female;
        info.hair = 2;
        info.armour = 3;
        info.weapon = 201;
        info.weapon_effect = 7;
        info.wing_effect = 6;
        info.transform_type = -1;
        info.riding_mount = false;
        info.fishing = true;

        let entity = world_entity_from_object_player_info(&info, None);
        assert_eq!(entity.riding_mount, Some(false));
        assert_eq!(entity.fishing, Some(true));
        let sprite = entity.sprite.expect("remote appearance sprite");
        assert_eq!(sprite.body_library, "CArmour/03");
        assert_eq!(sprite.hair_library.as_deref(), Some("CHair/02"));
        assert_eq!(sprite.weapon_library.as_deref(), Some("ARWeapon/01"));
        assert_eq!(sprite.alt_body_library.as_deref(), Some("ARArmour/03"));
        assert_eq!(sprite.alt_hair_library.as_deref(), Some("ARHair/02"));
        assert_eq!(sprite.alt_weapon_library.as_deref(), Some("ARWeapon/01 S"));
        assert_eq!(sprite.frame_base_offset, 808);
        assert_eq!(sprite.weapon_frame_offset, Some(416));
        assert_eq!(sprite.alt_frame_base_offset, Some(352));
        assert_eq!(sprite.alt_weapon_frame_offset, Some(352));

        info.transform_type = 4;
        let transformed = world_entity_from_object_player_info(&info, None)
            .sprite
            .expect("transformed remote sprite");
        assert_eq!(transformed.body_library, "Transform/04");
        assert_eq!(transformed.hair_library, None);
        assert_eq!(transformed.weapon_library, None);

        info.class = MirClass::Assassin;
        info.gender = MirGender::Female;
        info.weapon = -1;
        info.transform_type = -1;
        let empty_hand_assassin = world_entity_from_object_player_info(&info, None)
            .sprite
            .expect("empty-hand assassin sprite");
        assert_eq!(
            empty_hand_assassin.alt_body_library.as_deref(),
            Some("AArmour/03")
        );
        assert_eq!(
            empty_hand_assassin.alt_hair_library.as_deref(),
            Some("AHair/02")
        );
        assert_eq!(empty_hand_assassin.alt_weapon_library, None);
        assert_eq!(empty_hand_assassin.alt_weapon_library_secondary, None);
        assert_eq!(empty_hand_assassin.alt_frame_base_offset, Some(512));

        info.transform_type = 6;
        info.riding_mount = true;
        info.mount_type = 7;
        let hidden_independent_mount = world_entity_from_object_player_info(&info, None)
            .sprite
            .expect("transform-mounted sprite with hidden independent mount");
        assert_eq!(hidden_independent_mount.body_library, "TransformRide2/06");
        assert_eq!(hidden_independent_mount.mount_library, None);
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
    fn shared_zone_state_filters_stale_dead_entity_packets_for_owner_stream() {
        let mut state = SharedInProcessZoneState::new();

        state.sync_map_layer(
            "0".to_string(),
            vec![shared_monster_entity(77), shared_monster_entity(88)],
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

        let mut packets = vec![
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 331, y: 270 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 80,
                    expire: 0,
                },
            },
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 77,
                    location: Point { x: 329, y: 269 },
                    direction: MirDirection::Down,
                    kind: 0,
                },
            },
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 88,
                    position: Point { x: 332, y: 270 },
                    direction: MirDirection::Right,
                },
            },
        ];

        state.filter_stale_dead_entity_packets("0", &mut packets);

        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == 77
        )));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 77 && info.percent > 0
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == 77
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == 88
        )));
    }

    #[test]
    fn shared_owner_dead_entity_packet_filter_drops_stale_transforms_after_death() {
        let mut dead_ids = BTreeSet::new();
        let mut packets = vec![
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 0,
                    expire: 0,
                },
            },
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 331, y: 270 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 77,
                    percent: 80,
                    expire: 0,
                },
            },
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 77,
                    location: Point { x: 329, y: 269 },
                    direction: MirDirection::Down,
                    kind: 0,
                },
            },
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 88,
                    position: Point { x: 332, y: 270 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::ObjectRevived {
                info: ObjectRevivedInfo {
                    object_id: 77,
                    effect: true,
                },
            },
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 77,
                    position: Point { x: 333, y: 272 },
                    direction: MirDirection::Down,
                },
            },
        ];

        filter_stale_owner_dead_entity_packets(&mut dead_ids, &mut packets);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 77 && info.percent == 0
        )));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == 77 && movement.position.x == 331
        )));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 77 && info.percent > 0
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == 77
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == 88
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRevived { info } if info.object_id == 77
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == 77 && movement.position.x == 333
        )));
        assert!(dead_ids.is_empty());
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
        let current_zone_object_id = state.upsert_player(
            current_key.clone(),
            "Current",
            "0".to_string(),
            shared_picker_entity(1, 329, 269),
            10,
        );
        state.upsert_player(
            remote_key.clone(),
            "Remote",
            "0".to_string(),
            shared_picker_entity(2, 330, 269),
            10,
        );
        state
            .zone_session_keys
            .insert(current_session_id.clone(), current_key.clone());
        state
            .zone_session_keys
            .insert(remote_session_id.clone(), remote_key.clone());

        let drop = shared_gold_drop(9101, 329, 269, Some(current_zone_object_id), Some(100));
        let award = ZoneMonsterKillAward {
            monster_object_id: 9100,
            killed_at_ms: 1_000,
            monster_name: "Field Wasp".to_string(),
            experience: 6,
            drops: vec![drop.clone()],
            boss_audit: None,
        };
        let (_, _, _, _, current_awards, _, _) = state.dispatch_zone_outbounds(
            vec![
                ZoneOutbound::ToMany {
                    session_ids: vec![current_session_id.clone(), remote_session_id.clone()],
                    packets: vec![ground_drop_spawn_packet(&drop)],
                },
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
        state.apply_shared_entity_packets("0", &[ground_drop_spawn_packet(&drop)]);
        let shared_drop = state
            .maps
            .get("0")
            .and_then(|map| map.ground_drops.get(&drop.object_id))
            .expect("kill award should replace the legacy packet snapshot");
        assert_eq!(shared_drop, &drop);
    }

    #[test]
    fn shared_in_process_runtime_emits_gain_experience_after_kill_award_commit() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "zone-kill-award", "Blade");
        let before_experience = runtime.inner.world_snapshot().player_experience;

        let packets = runtime.apply_zone_monster_kill_awards(vec![ZoneMonsterKillAward {
            monster_object_id: 9100,
            killed_at_ms: 1_000,
            monster_name: "Field Wasp".to_string(),
            experience: 6,
            drops: Vec::new(),
            boss_audit: None,
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
                    killed_at_ms: 1_000,
                    monster_name: "Field Wasp".to_string(),
                    experience: 6,
                    drops: Vec::new(),
                    boss_audit: None,
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
                    components: Vec::new(),
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
        let boss_audit = ZoneBossRewardAudit {
            reward_owner_session_id: SessionId::new("inventory-idempotent-owner:0"),
            last_hit_session_id: SessionId::new("other-account:7"),
            damage_contributions: BTreeMap::from([
                (SessionId::new("inventory-idempotent-owner:0"), 60),
                (SessionId::new("other-account:7"), 40),
            ]),
        };

        let award_envelope = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::MonsterKillAward(ZoneMonsterKillAward {
                monster_object_id: 9100,
                killed_at_ms: 1_000,
                monster_name: "Field Wasp".to_string(),
                experience: 6,
                drops: Vec::new(),
                boss_audit: Some(boss_audit.clone()),
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
        assert_eq!(
            service.boss_reward_audits(),
            vec![boss_audit.clone()],
            "an idempotent retry must not duplicate the Boss reward audit"
        );

        let respawn_award = service.commit(
            &mut runtime.inner,
            SharedAccountInventoryCommandEnvelope {
                identity: identity.clone(),
                command: SharedAccountInventoryCommand::MonsterKillAward(ZoneMonsterKillAward {
                    monster_object_id: 9100,
                    killed_at_ms: 2_000,
                    monster_name: "Field Wasp".to_string(),
                    experience: 6,
                    drops: Vec::new(),
                    boss_audit: None,
                }),
            },
        );
        assert!(respawn_award.committed);
        assert_eq!(
            runtime.inner.world_snapshot().player_experience,
            before_experience + 12,
            "a respawned monster reuses its object id but is a distinct reward event"
        );
        assert_eq!(service.boss_reward_audits(), vec![boss_audit]);

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
                components: Vec::new(),
            },
        };
        let same = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonSkeleton,
                request_id: 42,
                components: Vec::new(),
            },
        };
        let distinct_request = SharedAccountInventoryCommandEnvelope {
            identity: identity.clone(),
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonSkeleton,
                request_id: 43,
                components: Vec::new(),
            },
        };
        let distinct_spell = SharedAccountInventoryCommandEnvelope {
            identity,
            command: SharedAccountInventoryCommand::SkillItemConsume {
                spell: Spell::SummonShinsu,
                request_id: 42,
                components: Vec::new(),
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
                SharedAccountInventoryCommand::GoldDrop { amount, .. } => {
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::GoldDrop,
                        committed: true,
                        packets: vec![ServerPacket::LoseGold { gold: amount }],
                    }
                }
                SharedAccountInventoryCommand::InventoryItemDrop { drop, .. } => {
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::InventoryItemDrop,
                        committed: true,
                        packets: vec![ServerPacket::DropItem {
                            unique_id: drop.unique_id,
                            count: u16::try_from(drop.quantity).unwrap_or(u16::MAX),
                            hero_inventory: drop.hero_inventory,
                            success: true,
                        }],
                    }
                }
                SharedAccountInventoryCommand::GroundDropPickup(_)
                | SharedAccountInventoryCommand::GroundDropClaimPickup { .. } => {
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

    #[derive(Debug)]
    struct EconomyContextFenceProbe {
        contexts: Arc<Mutex<Vec<Option<SharedAccountInventoryExecutionContext>>>>,
    }

    impl SharedAccountInventoryService for EconomyContextFenceProbe {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn commit_fenced(
            &self,
            runtime: &mut InProcessWorldRuntime,
            context: Option<&SharedAccountInventoryExecutionContext>,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryCommitOutcome {
            self.contexts
                .lock()
                .expect("economy fence contexts should lock")
                .push(context.cloned());
            let SharedAccountInventoryCommand::GoldDrop { .. } = &envelope.command else {
                panic!("economy fence probe only accepts GoldDrop")
            };
            if context.is_none_or(|context| !context.external_commit_authorized) {
                return SharedAccountInventoryCommitOutcome::Deferred {
                    receipt: SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::GoldDrop,
                        committed: false,
                        packets: Vec::new(),
                    },
                };
            }
            SharedAccountInventoryCommitOutcome::Confirmed(
                InProcessAccountInventoryService::new().commit(runtime, envelope),
            )
        }
    }

    #[test]
    fn hosted_owner_only_fences_authenticated_active_production_economy() {
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let service = Arc::new(EconomyContextFenceProbe {
            contexts: Arc::clone(&contexts),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "hosted-economy-fence", "FenceOwner");
        let mut checkpoint = runtime
            .inner
            .active_character_checkpoint()
            .expect("active character checkpoint");
        checkpoint.gold = 100;
        runtime
            .inner
            .restore_active_character_checkpoint(&checkpoint)
            .expect("seed test gold");
        let starting_gold = runtime.world_snapshot().gold;
        let client = HostedZoneOwnerCommandClient::new(Box::new(runtime));
        let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));

        let direct = client
            .execute_request(
                ZoneOwnerCommandRequest::direct(
                    lease.clone(),
                    WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 1 }),
                )
                .with_source_sequence(41),
            )
            .expect("direct command should fail closed without a durable context");
        assert!(direct.packets.is_empty());
        assert_eq!(client.world_snapshot().unwrap().gold, starting_gold);

        let unauthenticated = client
            .execute_request(
                ZoneOwnerCommandRequest::production_player(
                    lease.clone(),
                    false,
                    WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 1 }),
                )
                .with_source_sequence(42),
            )
            .expect("unauthenticated production command should be rejected");
        assert!(!unauthenticated
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
        assert_eq!(client.world_snapshot().unwrap().gold, starting_gold);

        let replay = client
            .execute_replay_request(
                ZoneOwnerCommandRequest::production_player(
                    lease.clone(),
                    true,
                    WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 1 }),
                )
                .with_source_sequence(43),
            )
            .expect("standby replay should apply without external economy authority");
        assert!(replay.packets.is_empty());
        assert_eq!(client.world_snapshot().unwrap().gold, starting_gold);

        let active = client
            .execute_request(
                ZoneOwnerCommandRequest::production_player(
                    lease,
                    true,
                    WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 1 }),
                )
                .with_source_sequence(44),
            )
            .expect("active authenticated production command should commit");
        assert!(active
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 1 })));
        assert_eq!(client.world_snapshot().unwrap().gold, starting_gold - 1);

        let contexts = contexts.lock().expect("economy fence contexts should lock");
        assert_eq!(contexts.len(), 4);
        assert_eq!(
            contexts[0], None,
            "Direct mode is not externally authorized"
        );
        assert_eq!(
            contexts[1], None,
            "unauthenticated production mode is not externally authorized"
        );
        assert_eq!(
            contexts[2], None,
            "standby replay is not externally authorized"
        );
        let active_context = contexts[3]
            .as_ref()
            .expect("active production command should receive a context");
        assert_eq!(active_context.source_sequence, 44);
        assert_eq!(active_context.fencing_generation, 1);
        assert!(active_context.external_commit_authorized);
    }

    #[derive(Debug)]
    struct GroundDropProjectionReconciliationProbe {
        calls: Arc<Mutex<usize>>,
        pending: Arc<Mutex<bool>>,
    }

    impl SharedAccountInventoryService for GroundDropProjectionReconciliationProbe {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn reconcile_ground_drop_projections_fenced(
            &self,
            _runtime: &mut InProcessWorldRuntime,
            _context: Option<&SharedAccountInventoryExecutionContext>,
        ) -> Vec<ServerPacket> {
            *self
                .calls
                .lock()
                .expect("ground projection probe should lock") += 1;
            Vec::new()
        }

        fn has_pending_ground_drop_projection_fenced(
            &self,
            _runtime: &InProcessWorldRuntime,
            _context: Option<&SharedAccountInventoryExecutionContext>,
        ) -> bool {
            *self
                .pending
                .lock()
                .expect("ground projection pending should lock")
        }
    }

    #[derive(Debug)]
    struct TradeProjectionReconciliationProbe {
        calls: Arc<Mutex<usize>>,
        pending: Arc<Mutex<bool>>,
    }

    impl SharedAccountInventoryService for TradeProjectionReconciliationProbe {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn reconcile_trade_projections_fenced(
            &self,
            _runtime: &mut InProcessWorldRuntime,
            _context: Option<&SharedAccountInventoryExecutionContext>,
        ) -> Vec<ServerPacket> {
            *self.calls.lock().expect("projection probe should lock") += 1;
            Vec::new()
        }

        fn has_pending_trade_projection_fenced(
            &self,
            _runtime: &InProcessWorldRuntime,
            _context: Option<&SharedAccountInventoryExecutionContext>,
        ) -> bool {
            *self.pending.lock().expect("projection pending should lock")
        }
    }

    #[derive(Debug)]
    struct RecordingTradeSettlementService {
        bootstraps: Arc<Mutex<usize>>,
        trades: Arc<Mutex<Vec<(SharedTradeOffer, SharedTradeOffer)>>>,
    }

    impl SharedAccountInventoryService for RecordingTradeSettlementService {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn bootstrap_fenced(
            &self,
            _runtime: &InProcessWorldRuntime,
            _context: Option<&SharedAccountInventoryExecutionContext>,
        ) -> bool {
            *self
                .bootstraps
                .lock()
                .expect("trade bootstrap count should lock") += 1;
            true
        }

        fn settle_trade_fenced(
            &self,
            _context: Option<&SharedAccountInventoryExecutionContext>,
            first: &SharedTradeOffer,
            second: &SharedTradeOffer,
        ) -> SharedTradeSettlementOutcome {
            self.trades
                .lock()
                .expect("recorded trades should lock")
                .push((first.clone(), second.clone()));
            SharedTradeSettlementOutcome::Committed
        }
    }

    #[derive(Debug)]
    struct UnknownThenRejectedTradeSettlementService {
        unresolved: Arc<Mutex<bool>>,
        calls: Arc<Mutex<usize>>,
    }

    impl SharedAccountInventoryService for UnknownThenRejectedTradeSettlementService {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn settle_trade_fenced(
            &self,
            context: Option<&SharedAccountInventoryExecutionContext>,
            first: &SharedTradeOffer,
            second: &SharedTradeOffer,
        ) -> SharedTradeSettlementOutcome {
            *self.calls.lock().expect("settlement calls should lock") += 1;
            if context.is_none() {
                return SharedTradeSettlementOutcome::Deferred;
            }
            if *self.unresolved.lock().expect("settlement mode should lock") {
                let mut nonces = [
                    first.settlement_nonce.as_str(),
                    second.settlement_nonce.as_str(),
                ];
                nonces.sort_unstable();
                SharedTradeSettlementOutcome::OutcomeUnknown {
                    idempotency_key: format!("test-commit-ack-unknown:{}:{}", nonces[0], nonces[1]),
                    execution_context: context
                        .expect("unknown trade outcome requires a test context")
                        .clone(),
                }
            } else {
                SharedTradeSettlementOutcome::Rejected
            }
        }
    }

    #[derive(Debug)]
    struct UnknownThenCommittedTradeSettlementService {
        unresolved: Arc<Mutex<bool>>,
        calls: Arc<Mutex<usize>>,
    }

    impl SharedAccountInventoryService for UnknownThenCommittedTradeSettlementService {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn settle_trade_fenced(
            &self,
            context: Option<&SharedAccountInventoryExecutionContext>,
            first: &SharedTradeOffer,
            second: &SharedTradeOffer,
        ) -> SharedTradeSettlementOutcome {
            *self.calls.lock().expect("settlement calls should lock") += 1;
            let Some(context) = context else {
                return SharedTradeSettlementOutcome::Deferred;
            };
            if *self.unresolved.lock().expect("settlement mode should lock") {
                let mut nonces = [
                    first.settlement_nonce.as_str(),
                    second.settlement_nonce.as_str(),
                ];
                nonces.sort_unstable();
                SharedTradeSettlementOutcome::OutcomeUnknown {
                    idempotency_key: format!("test-commit-ack-unknown:{}:{}", nonces[0], nonces[1]),
                    execution_context: context.clone(),
                }
            } else {
                SharedTradeSettlementOutcome::Committed
            }
        }
    }

    #[derive(Debug)]
    struct UnknownThenCommittedGroundDropService {
        unresolved: Arc<Mutex<bool>>,
        calls: Arc<Mutex<usize>>,
    }

    impl SharedAccountInventoryService for UnknownThenCommittedGroundDropService {
        fn commit(
            &self,
            runtime: &mut InProcessWorldRuntime,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryTransactionReceipt {
            InProcessAccountInventoryService::new().commit(runtime, envelope)
        }

        fn commit_fenced(
            &self,
            runtime: &mut InProcessWorldRuntime,
            context: Option<&SharedAccountInventoryExecutionContext>,
            envelope: SharedAccountInventoryCommandEnvelope,
        ) -> SharedAccountInventoryCommitOutcome {
            if !matches!(
                &envelope.command,
                SharedAccountInventoryCommand::GroundDropPickup(_)
                    | SharedAccountInventoryCommand::GroundDropClaimPickup { .. }
            ) {
                return SharedAccountInventoryCommitOutcome::Confirmed(
                    InProcessAccountInventoryService::new().commit(runtime, envelope),
                );
            }
            *self
                .calls
                .lock()
                .expect("ground settlement calls should lock") += 1;
            if context.is_none() {
                return SharedAccountInventoryCommitOutcome::Deferred {
                    receipt: SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
                        committed: false,
                        packets: Vec::new(),
                    },
                };
            }
            if *self
                .unresolved
                .lock()
                .expect("ground settlement mode should lock")
            {
                let idempotency_key = envelope.stable_idempotency_key();
                SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                    idempotency_key,
                    execution_context: context
                        .expect("unknown ground outcome requires a test context")
                        .clone(),
                    receipt: SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
                        committed: false,
                        packets: Vec::new(),
                    },
                }
            } else {
                SharedAccountInventoryCommitOutcome::Confirmed(
                    InProcessAccountInventoryService::new().commit(runtime, envelope),
                )
            }
        }
    }

    #[test]
    fn active_character_retries_pending_ground_drop_projection_then_caches_clear_state() {
        let calls = Arc::new(Mutex::new(0));
        let pending = Arc::new(Mutex::new(true));
        let service = Arc::new(GroundDropProjectionReconciliationProbe {
            calls: calls.clone(),
            pending: pending.clone(),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "drop-reconcile-on-start", "DropOwner");
        assert_eq!(
            *calls.lock().expect("probe count should lock"),
            1,
            "StartGame should reconcile immediately after the Zone join"
        );

        runtime.execute(WorldCommand::Tick).expect("pending tick");
        assert_eq!(*calls.lock().expect("probe count should lock"), 2);
        runtime.execute(WorldCommand::Tick).expect("retry tick");
        assert_eq!(*calls.lock().expect("probe count should lock"), 3);

        *pending.lock().expect("projection pending should lock") = false;
        runtime.execute(WorldCommand::Tick).expect("clear tick");
        assert_eq!(*calls.lock().expect("probe count should lock"), 4);
        runtime
            .execute(WorldCommand::Tick)
            .expect("cached clear tick");
        assert_eq!(
            *calls.lock().expect("probe count should lock"),
            4,
            "a clear identity should not poll PostgreSQL on ordinary ticks"
        );
    }

    #[test]
    fn commit_ack_unknown_ground_claim_is_checkpointed_and_retried_without_restore() {
        let unresolved = Arc::new(Mutex::new(true));
        let calls = Arc::new(Mutex::new(0));
        let service = Arc::new(UnknownThenCommittedGroundDropService {
            unresolved: unresolved.clone(),
            calls: calls.clone(),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(Arc::clone(&zone_state), service);
        start_new_runtime(&mut runtime, "ground-ack-unknown", "ClaimOwner");
        runtime.set_economy_execution_context(Some(SharedAccountInventoryExecutionContext {
            zone_id: ZoneId::new("test-shared-zone"),
            fencing_generation: 1,
            source_sequence: 1,
            created_at_ms: 1_000,
            external_commit_authorized: true,
        }));
        let self_entity = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        let drop = shared_gold_drop(91_001, self_entity.x, self_entity.y, None, None);
        let object_id = drop.object_id;
        let session_id = runtime
            .current_zone_session_id()
            .expect("active shared Zone session");
        let key = runtime
            .current_presence_key()
            .expect("active shared Zone presence");
        let initial_gold = runtime.inner.world_snapshot().gold;
        runtime.restore_zone_ground_drop_claim(drop.clone());
        let _ = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncGroundDrops {
                session_id: session_id.clone(),
                drops: vec![drop],
                now_ms: 1_000,
            },
            false,
        );

        let first_packets = runtime.dispatch_zone_player_command(
            ZoneCommand::ClaimGroundDrop {
                session_id,
                object_id: Some(object_id),
                target: Point {
                    x: self_entity.x,
                    y: self_entity.y,
                },
                group_members: Vec::new(),
                now_ms: 1_001,
            },
            false,
        );

        assert_eq!(*calls.lock().expect("ground settlement calls"), 1);
        assert_eq!(runtime.inner.world_snapshot().gold, initial_gold);
        assert!(first_packets
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
        let checkpoint = {
            let state = zone_state.lock().expect("shared Zone state should lock");
            assert!(!state.pending_zone_ground_drop_claims.contains_key(&key));
            assert!(state
                .zone_manager
                .pending_ground_drop_claim_tickets()
                .is_empty());
            let settlement = state
                .unresolved_ground_drop_settlement_for_presence(&key)
                .expect("unknown claim must detach into recovery authority");
            assert_eq!(
                settlement.execution_context,
                Some(SharedAccountInventoryExecutionContext {
                    zone_id: ZoneId::new("test-shared-zone"),
                    fencing_generation: 1,
                    source_sequence: 1,
                    created_at_ms: 1_000,
                    external_commit_authorized: true,
                })
            );
            assert!(state
                .zone_manager
                .has_detached_ground_drop_claim_ticket(&settlement.zone_key, &settlement.ticket,));
            state.checkpoint().expect("unknown claim checkpoint")
        };
        SharedInProcessZoneState::restore(checkpoint)
            .expect("unknown claim must survive checkpoint restore");
        let world_checkpoint = zone_state
            .lock()
            .expect("shared Zone state should lock")
            .world_checkpoint()
            .expect("world-only unknown claim checkpoint");
        assert!(world_checkpoint.zone_sessions.is_empty());
        assert!(world_checkpoint.zone_session_keys.is_empty());
        assert!(world_checkpoint.players.is_empty());
        assert!(world_checkpoint.pending_zone_ground_drop_claims.is_empty());
        assert_eq!(world_checkpoint.unresolved_ground_drop_settlements.len(), 1);
        let restored_world = SharedInProcessZoneState::restore(world_checkpoint)
            .expect("world-only unknown claim must restore");
        assert!(restored_world.zone_sessions.is_empty());
        assert!(restored_world.players.is_empty());
        assert_eq!(restored_world.unresolved_ground_drop_settlements.len(), 1);
        let detached = restored_world
            .unresolved_ground_drop_settlements
            .values()
            .next()
            .expect("detached recovery record");
        assert!(restored_world
            .zone_manager
            .has_detached_ground_drop_claim_ticket(&detached.zone_key, &detached.ticket));

        runtime.set_economy_execution_context(None);
        let deferred_packets = runtime.apply_pending_shared_trade_packets();
        assert!(deferred_packets.is_empty());
        assert_eq!(*calls.lock().expect("ground settlement calls"), 1);
        assert_eq!(runtime.inner.world_snapshot().gold, initial_gold);
        assert!(zone_state
            .lock()
            .expect("shared Zone state should lock")
            .unresolved_ground_drop_settlement_for_presence(&key)
            .is_some());

        runtime.set_economy_execution_context(Some(SharedAccountInventoryExecutionContext {
            zone_id: ZoneId::new("test-shared-zone"),
            fencing_generation: 1,
            source_sequence: 2,
            created_at_ms: 1_001,
            external_commit_authorized: true,
        }));
        *unresolved.lock().expect("ground settlement mode") = false;
        let retry_packets = runtime.apply_pending_shared_trade_packets();
        assert_eq!(*calls.lock().expect("ground settlement calls"), 2);
        assert_eq!(runtime.inner.world_snapshot().gold, initial_gold + 25);
        assert!(retry_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold } if *gold == 25)));
        {
            let state = zone_state.lock().expect("shared Zone state should lock");
            assert!(!state.pending_zone_ground_drop_claims.contains_key(&key));
            assert!(state
                .zone_manager
                .pending_ground_drop_claim_tickets()
                .is_empty());
            assert!(state.unresolved_ground_drop_settlements.is_empty());
        }

        assert!(runtime.apply_pending_shared_trade_packets().is_empty());
        assert_eq!(*calls.lock().expect("ground settlement calls"), 2);
        assert_eq!(runtime.inner.world_snapshot().gold, initial_gold + 25);
    }

    #[test]
    fn deferred_trade_recovery_keeps_authority_then_retries_the_real_key() {
        let unresolved = Arc::new(Mutex::new(true));
        let calls = Arc::new(Mutex::new(0));
        let service = Arc::new(UnknownThenRejectedTradeSettlementService {
            unresolved: Arc::clone(&unresolved),
            calls: Arc::clone(&calls),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(Arc::clone(&zone_state), service);
        start_new_runtime(&mut runtime, "trade-deferred-recovery", "DeferredOwner");
        let identity = runtime
            .inner
            .active_identity()
            .expect("active trade owner identity");
        let first_key = ZonePresenceKey::from_identity(&identity);
        let second_key = ZonePresenceKey {
            account_id: "trade-deferred-partner".to_string(),
            character_index: 0,
        };
        let first_offer = SharedTradeOffer {
            settlement_nonce: "00000000000000000000000000000091".to_string(),
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
            character_name: identity.character_name.clone(),
            partner_name: "DeferredPartner".to_string(),
            gold: 30,
            items: Vec::new(),
        };
        let second_offer = SharedTradeOffer {
            settlement_nonce: "00000000000000000000000000000092".to_string(),
            account_id: second_key.account_id.clone(),
            character_index: second_key.character_index,
            character_name: "DeferredPartner".to_string(),
            partner_name: identity.character_name,
            gold: 0,
            items: Vec::new(),
        };
        let mut nonces = [
            first_offer.settlement_nonce.as_str(),
            second_offer.settlement_nonce.as_str(),
        ];
        nonces.sort_unstable();
        let idempotency_key = format!("test-commit-ack-unknown:{}:{}", nonces[0], nonces[1]);
        zone_state
            .lock()
            .expect("shared Zone state should lock")
            .retain_unresolved_trade_settlement(UnresolvedSharedTradeSettlement {
                idempotency_key: idempotency_key.clone(),
                execution_context: Some(SharedAccountInventoryExecutionContext {
                    zone_id: ZoneId::new("test-origin-zone"),
                    fencing_generation: 1,
                    source_sequence: 90,
                    created_at_ms: 9_000,
                    external_commit_authorized: true,
                }),
                first_key: first_key.clone(),
                second_key: second_key.clone(),
                first_offer,
                second_offer,
            })
            .expect("fixture unresolved trade should insert");

        runtime.set_economy_execution_context(None);
        assert!(runtime.resolve_unresolved_trade_settlement());
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 0);
        {
            let state = zone_state.lock().expect("shared Zone state should lock");
            assert_eq!(state.unresolved_trade_settlements.len(), 1);
            assert!(state.pending_trade_rollbacks.is_empty());
            assert!(state.pending_trade_deliveries.is_empty());
        }

        runtime.set_economy_execution_context(Some(SharedAccountInventoryExecutionContext {
            zone_id: ZoneId::new("test-shared-zone"),
            fencing_generation: 2,
            source_sequence: 91,
            created_at_ms: 9_100,
            external_commit_authorized: true,
        }));
        assert!(runtime.resolve_unresolved_trade_settlement());
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 1);
        {
            let state = zone_state.lock().expect("shared Zone state should lock");
            let retained = state
                .unresolved_trade_settlement_for_presence(&first_key)
                .expect("ordered unknown result should remain retained");
            assert_eq!(retained.idempotency_key, idempotency_key);
            assert!(state.pending_trade_rollbacks.is_empty());
            assert!(state.pending_trade_deliveries.is_empty());
        }

        *unresolved.lock().expect("settlement mode should lock") = false;
        assert!(!runtime.resolve_unresolved_trade_settlement());
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 2);
        let state = zone_state.lock().expect("shared Zone state should lock");
        assert!(state.unresolved_trade_settlements.is_empty());
        assert!(state
            .pending_trade_rollbacks
            .get(&first_key)
            .is_some_and(|offers| offers.len() == 1));
        assert!(state
            .pending_trade_rollbacks
            .get(&second_key)
            .is_some_and(|offers| offers.len() == 1));
    }

    #[test]
    fn unknown_ground_claim_survives_process_restart_and_credits_once_after_new_login() {
        let unresolved = Arc::new(Mutex::new(true));
        let calls = Arc::new(Mutex::new(0));
        let service = Arc::new(UnknownThenCommittedGroundDropService {
            unresolved: Arc::clone(&unresolved),
            calls: Arc::clone(&calls),
        }) as SharedAccountInventoryServiceHandle;
        let first_factory = Arc::new(
            SharedInProcessZoneRuntimeFactory::with_account_inventory_service(Arc::clone(&service)),
        );
        let first_registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::clone(&first_factory) as SharedZoneRuntimeFactory,
        );
        let config = GatewayConfig::default();
        let mut owner = GatewaySession::new_with_zone_registry(config.clone(), &first_registry);
        let mut claimant = GatewaySession::new_with_zone_registry(config.clone(), &first_registry);
        start_demo_character(&mut owner);
        start_new_character(&mut claimant, "ground-restart-claimant", "RestartClaimant");
        let claimant_identity = claimant
            .active_identity()
            .expect("claimant should have an active character");
        let character_index = claimant_identity.character_index;
        let starting_gold = claimant.world_snapshot().gold;

        owner.handle_packet(ClientPacket::DropGold { amount: 25 });
        let shared_drop = claimant
            .world_snapshot()
            .ground_drops
            .into_iter()
            .find(|drop| {
                matches!(
                    &drop.loot,
                    GroundDropLootSnapshot::Gold { amount } if *amount == 25
                )
            })
            .expect("claimant should observe the shared gold drop");
        let object_id = shared_drop.object_id;
        claimant.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));
        let unknown_packets = claimant
            .execute_production_player_command(true, WorldCommand::PickUp { object_id })
            .expect("ordered claimant pickup should execute")
            .packets;

        assert!(
            unknown_packets
                .iter()
                .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })),
            "unknown pickup must not deliver gold: {unknown_packets:#?}"
        );
        assert_eq!(claimant.world_snapshot().gold, starting_gold);
        assert_eq!(*calls.lock().expect("ground settlement calls"), 1);

        let (zone_key, ticket) = {
            let resources = first_factory.resources_for_zone(&ZoneId::primary());
            let state = resources
                .zone_state
                .lock()
                .expect("first factory zone state should lock");
            let settlement = state
                .unresolved_ground_drop_settlements
                .values()
                .next()
                .expect("unknown pickup should retain one recovery record");
            assert!(state
                .zone_manager
                .has_detached_ground_drop_claim_ticket(&settlement.zone_key, &settlement.ticket,));
            (settlement.zone_key.clone(), settlement.ticket.clone())
        };
        let world_checkpoint_bytes = first_factory
            .world_checkpoint_bytes()
            .expect("world-only checkpoint should preserve unknown pickup");

        drop(claimant);
        drop(owner);
        assert_eq!(
            *calls.lock().expect("ground settlement calls"),
            1,
            "teardown and Drop must not retry without an economy command context"
        );

        let restored_factory =
            Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service));
        assert_eq!(
            restored_factory
                .install_world_checkpoint_bytes(&world_checkpoint_bytes)
                .expect("fresh factory should install world-only checkpoint"),
            1
        );
        {
            let resources = restored_factory.resources_for_zone(&ZoneId::primary());
            let state = resources
                .zone_state
                .lock()
                .expect("restored factory zone state should lock");
            assert_eq!(state.unresolved_ground_drop_settlements.len(), 1);
            assert!(state
                .zone_manager
                .has_detached_ground_drop_claim_ticket(&zone_key, &ticket));
        }

        *unresolved.lock().expect("ground settlement mode") = false;
        let restored_registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::clone(&restored_factory) as SharedZoneRuntimeFactory,
        );
        let mut resumed = GatewaySession::new_with_zone_registry(config, &restored_registry);
        resumed.handle_packet(ClientPacket::Login {
            account_id: claimant_identity.account_id.clone(),
            password: claimant_identity.account_id.clone(),
        });
        let recovery_packets = resumed
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::StartGame { character_index }),
            )
            .expect("ordered StartGame should recover the durable pickup")
            .packets;

        assert!(recovery_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 25 })));
        assert_eq!(*calls.lock().expect("ground settlement calls"), 2);
        assert_eq!(resumed.world_snapshot().gold, starting_gold + 25);
        {
            let resources = restored_factory.resources_for_zone(&ZoneId::primary());
            let state = resources
                .zone_state
                .lock()
                .expect("resolved factory zone state should lock");
            assert!(state.unresolved_ground_drop_settlements.is_empty());
            let zone = state
                .zone_manager
                .zone(&zone_key)
                .expect("restored Zone should remain present");
            assert!(!zone.has_ground_drop(object_id));
            assert!(
                zone.has_detached_ground_drop_claim_ticket(&ticket),
                "the committed object's tombstone must continue blocking restoration"
            );
        }

        let later_packets = resumed.handle_packet(ClientPacket::KeepAlive { time: 73 });
        assert!(later_packets
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
        assert_eq!(*calls.lock().expect("ground settlement calls"), 2);
        assert_eq!(resumed.world_snapshot().gold, starting_gold + 25);
    }

    #[test]
    fn unknown_trade_survives_process_restart_and_delivers_both_sides_once_after_new_logins() {
        let unresolved = Arc::new(Mutex::new(true));
        let calls = Arc::new(Mutex::new(0));
        let service = Arc::new(UnknownThenCommittedTradeSettlementService {
            unresolved: Arc::clone(&unresolved),
            calls: Arc::clone(&calls),
        }) as SharedAccountInventoryServiceHandle;
        let first_factory = Arc::new(
            SharedInProcessZoneRuntimeFactory::with_account_inventory_service(Arc::clone(&service)),
        );
        let first_registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::clone(&first_factory) as SharedZoneRuntimeFactory,
        );
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &first_registry);
        let mut second = GatewaySession::new_with_zone_registry(config.clone(), &first_registry);
        start_demo_character(&mut first);
        start_new_character(&mut second, "trade-restart-second", "RestartBob");
        let first_identity = first.active_identity().expect("first trade identity");
        let second_identity = second.active_identity().expect("second trade identity");

        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .into_iter()
            .find(|drop| {
                matches!(
                    &drop.loot,
                    GroundDropLootSnapshot::Gold { amount } if *amount == 100
                )
            })
            .expect("second party should observe funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);
        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;

        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        let first_confirm = first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert!(first_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 30 })));
        second.handle_packet(ClientPacket::TradeGold { amount: 40 });
        let unknown_packets = second
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::TradeConfirm { locked: true }),
            )
            .expect("authenticated ordered trade confirmation should execute")
            .packets;
        assert!(unknown_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 40 })));
        assert!(
            unknown_packets
                .iter()
                .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })),
            "unknown settlement must not deliver either offer: {unknown_packets:#?}"
        );
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 40);
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 1);
        first
            .save_active_character()
            .expect("first debited offer should persist before process restart");
        second
            .save_active_character()
            .expect("second debited offer should persist before process restart");

        let unresolved_authority = {
            let resources = first_factory.resources_for_zone(&ZoneId::primary());
            let state = resources
                .zone_state
                .lock()
                .expect("first factory Zone state should lock");
            let settlement = state
                .unresolved_trade_settlements
                .values()
                .next()
                .expect("unknown trade should retain one recovery record")
                .clone();
            assert!(settlement.execution_context.is_some());
            assert!(state.pending_trade_deliveries.is_empty());
            assert!(state.pending_trade_rollbacks.is_empty());
            settlement
        };
        let world_checkpoint_bytes = first_factory
            .world_checkpoint_bytes()
            .expect("world-only checkpoint should preserve unknown trade");

        drop(second);
        drop(first);
        assert_eq!(
            *calls.lock().expect("settlement calls should lock"),
            1,
            "teardown and Drop must not retry without an economy command context"
        );

        let restored_factory =
            Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service));
        assert_eq!(
            restored_factory
                .install_world_checkpoint_bytes(&world_checkpoint_bytes)
                .expect("fresh factory should install world-only checkpoint"),
            1
        );
        {
            let resources = restored_factory.resources_for_zone(&ZoneId::primary());
            let state = resources
                .zone_state
                .lock()
                .expect("restored factory Zone state should lock");
            assert_eq!(state.unresolved_trade_settlements.len(), 1);
            assert_eq!(
                state.unresolved_trade_settlements.values().next(),
                Some(&unresolved_authority),
                "restart must preserve the exact old key, context, parties, and offers"
            );
            assert!(state.pending_trade_deliveries.is_empty());
            assert!(state.pending_trade_rollbacks.is_empty());
        }

        *unresolved.lock().expect("settlement mode should lock") = false;
        let restored_registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::clone(&restored_factory) as SharedZoneRuntimeFactory,
        );
        let mut resumed_first =
            GatewaySession::new_with_zone_registry(config.clone(), &restored_registry);
        resumed_first.handle_packet(ClientPacket::Login {
            account_id: first_identity.account_id.clone(),
            password: "demo".to_string(),
        });
        let first_recovery = resumed_first
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::StartGame {
                    character_index: first_identity.character_index,
                }),
            )
            .expect("first party StartGame should resolve and deliver the trade")
            .packets;
        assert!(first_recovery
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 40 })));
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 2);
        assert_eq!(
            resumed_first.world_snapshot().gold,
            first_starting_gold - 30 + 40
        );

        let mut resumed_second = GatewaySession::new_with_zone_registry(config, &restored_registry);
        resumed_second.handle_packet(ClientPacket::Login {
            account_id: second_identity.account_id.clone(),
            password: second_identity.account_id.clone(),
        });
        let second_recovery = resumed_second
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::StartGame {
                    character_index: second_identity.character_index,
                }),
            )
            .expect("second party StartGame should consume the finalized delivery")
            .packets;
        assert!(second_recovery
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 2);
        assert_eq!(
            resumed_second.world_snapshot().gold,
            second_starting_gold - 40 + 30
        );

        {
            let resources = restored_factory.resources_for_zone(&ZoneId::primary());
            let state = resources
                .zone_state
                .lock()
                .expect("resolved factory Zone state should lock");
            assert!(state.unresolved_trade_settlements.is_empty());
            assert!(state.pending_trade_deliveries.is_empty());
            assert!(state.pending_trade_rollbacks.is_empty());
        }
        for packets in [
            resumed_first.handle_packet(ClientPacket::KeepAlive { time: 74 }),
            resumed_second.handle_packet(ClientPacket::KeepAlive { time: 75 }),
        ] {
            assert!(packets
                .iter()
                .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
        }
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 2);
        assert_eq!(
            resumed_first.world_snapshot().gold,
            first_starting_gold - 30 + 40
        );
        assert_eq!(
            resumed_second.world_snapshot().gold,
            second_starting_gold - 40 + 30
        );
    }

    #[test]
    fn active_character_reconciles_durable_trade_projection_once_without_local_trade_state() {
        let calls = Arc::new(Mutex::new(0));
        let pending = Arc::new(Mutex::new(false));
        let service = Arc::new(TradeProjectionReconciliationProbe {
            calls: calls.clone(),
            pending,
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "trade-reconcile-on-start", "TradeStart");
        assert!(!runtime.inner.has_active_shared_trade_state());
        assert_eq!(
            *calls.lock().expect("probe count should lock"),
            1,
            "StartGame should reconcile immediately after the Zone join"
        );

        runtime.execute(WorldCommand::Tick).expect("first tick");
        assert_eq!(*calls.lock().expect("probe count should lock"), 1);
        runtime.execute(WorldCommand::Tick).expect("second tick");
        assert_eq!(
            *calls.lock().expect("probe count should lock"),
            1,
            "an identity with no pending trade should not poll on every tick"
        );
    }

    #[test]
    fn unreconciled_character_blocks_trade_mutation_until_projection_query_succeeds() {
        let calls = Arc::new(Mutex::new(0));
        let pending = Arc::new(Mutex::new(true));
        let service = Arc::new(TradeProjectionReconciliationProbe {
            calls: calls.clone(),
            pending: pending.clone(),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "trade-reconcile-fail-closed", "TradeGuard");

        assert!(runtime.has_pending_durable_trade_projection());
        assert_eq!(
            *calls.lock().expect("probe count should lock"),
            1,
            "StartGame should make the first fail-closed reconciliation attempt"
        );
        runtime.execute(WorldCommand::Tick).expect("pending tick");
        assert_eq!(*calls.lock().expect("probe count should lock"), 2);
        assert!(runtime.has_pending_durable_trade_projection());

        *pending.lock().expect("projection pending should lock") = false;
        runtime.execute(WorldCommand::Tick).expect("recovered tick");
        assert_eq!(*calls.lock().expect("probe count should lock"), 3);
        assert!(!runtime.has_pending_durable_trade_projection());
    }

    #[test]
    fn durable_settlement_invalidates_clear_projection_cache_without_local_trade_state() {
        let calls = Arc::new(Mutex::new(0));
        let pending = Arc::new(Mutex::new(false));
        let service = Arc::new(TradeProjectionReconciliationProbe {
            calls: calls.clone(),
            pending: pending.clone(),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "trade-cache-invalidate", "TradeCache");

        runtime.execute(WorldCommand::Tick).expect("initial clear");
        assert_eq!(*calls.lock().expect("probe count should lock"), 1);
        assert!(!runtime.inner.has_active_shared_trade_state());
        assert!(!runtime.has_pending_durable_trade_projection());

        *pending.lock().expect("pending flag should lock") = true;
        runtime.note_durable_trade_projection_pending();
        assert!(
            runtime.has_pending_durable_trade_projection(),
            "a durable settlement must invalidate the old clear cache even after local trade clears"
        );

        let mutations = [
            ClientPacket::TradeRequest,
            ClientPacket::TradeReply {
                accept_invite: false,
            },
            ClientPacket::TradeGold { amount: 1 },
            ClientPacket::DepositTradeItem { from: 0, to: 0 },
            ClientPacket::RetrieveTradeItem { from: 0, to: 0 },
            ClientPacket::TradeConfirm { locked: false },
            ClientPacket::TradeCancel,
        ];
        for packet in mutations {
            assert!(runtime
                .execute(WorldCommand::ClientPacket(packet))
                .expect("guarded trade packet")
                .is_empty());
        }
        assert!(!runtime.inner.has_active_shared_trade_state());
        let calls_while_pending = *calls.lock().expect("probe count should lock");
        assert!(
            calls_while_pending > 1,
            "pending reconciliation must remain live across guarded commands"
        );

        *pending.lock().expect("pending flag should lock") = false;
        runtime
            .execute(WorldCommand::Tick)
            .expect("projection clears");
        assert_eq!(
            *calls.lock().expect("probe count should lock"),
            calls_while_pending + 1
        );
        assert!(!runtime.has_pending_durable_trade_projection());
    }

    #[test]
    fn pending_durable_trade_blocks_every_trade_state_mutation_packet() {
        let calls = Arc::new(Mutex::new(0));
        let pending = Arc::new(Mutex::new(false));
        let service = Arc::new(TradeProjectionReconciliationProbe {
            calls,
            pending: pending.clone(),
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "trade-pending-mutations", "TradeGuard");
        runtime
            .execute(WorldCommand::Tick)
            .expect("initial reconcile");

        runtime.inner.trade_request("Trader");
        runtime
            .inner
            .execute(WorldCommand::ClientPacket(ClientPacket::TradeReply {
                accept_invite: true,
            }))
            .expect("accept local fixture trade");
        runtime
            .inner
            .execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
                amount: 25,
            }))
            .expect("offer fixture gold");
        let (_, offer) = runtime.inner.shared_trade_confirm();
        let offer = offer.expect("completed fixture offer");
        let before = runtime.inner.world_snapshot();
        *pending.lock().expect("pending flag should lock") = true;

        let mutations = [
            ClientPacket::TradeRequest,
            ClientPacket::TradeReply {
                accept_invite: false,
            },
            ClientPacket::TradeGold { amount: 1 },
            ClientPacket::DepositTradeItem { from: 0, to: 0 },
            ClientPacket::RetrieveTradeItem { from: 0, to: 0 },
            ClientPacket::TradeConfirm { locked: false },
            ClientPacket::TradeCancel,
        ];
        for packet in mutations {
            assert!(runtime
                .execute(WorldCommand::ClientPacket(packet))
                .expect("guarded trade packet")
                .is_empty());
        }

        let after = runtime.inner.world_snapshot();
        let trade = after
            .stage5_systems
            .trade
            .expect("trade state is preserved");
        assert_eq!(after.gold, before.gold);
        assert_eq!(trade.settlement_nonce, offer.settlement_nonce);
        assert_eq!(trade.partner, offer.partner_name);
        assert_eq!(trade.offered_gold, offer.gold);
        assert!(trade.completed);
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
            killed_at_ms: 1_000,
            monster_name: "Field Wasp".to_string(),
            experience: 6,
            drops: Vec::new(),
            boss_audit: None,
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
        let claim_session_id = runtime
            .current_zone_session_id()
            .expect("active shared Zone session");
        let claim_ticket = GroundDropClaimTicket {
            claim_id: 1,
            object_id: gold_drop.object_id,
            drop_generation: 1,
            payload_digest: "actor-boundary-payload".to_string(),
            idempotency_key: "actor-boundary-claim".to_string(),
            session_id: claim_session_id,
            owner_object_id: gold_drop.owner_object_id,
            drop: gold_drop,
        };
        let (pickup_packets, canceled_claims) =
            runtime.apply_zone_ground_drop_claims(vec![claim_ticket]);
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
                        SharedAccountInventoryCommand::GroundDropClaimPickup {
                            drop,
                            claim_idempotency_key,
                        } if drop.object_id == 9101
                            && drop.name == "Actor Boundary Gold"
                            && claim_idempotency_key == "actor-boundary-claim"
                    )
            }));
        assert!(canceled_claims.contains(&9101));
        assert!(pickup_packets
            .values()
            .flatten()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
    }

    #[test]
    fn intelligent_creature_shared_pickup_uses_ticket_bound_account_service() {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let ground_drop_calls = Arc::new(Mutex::new(0));
        let service = Arc::new(RecordingAccountInventoryService {
            commands: commands.clone(),
            ground_drop_calls: ground_drop_calls.clone(),
            monster_award_calls: Arc::new(Mutex::new(0)),
            skill_item_calls: Arc::new(Mutex::new(0)),
            ground_drop_committed: true,
            monster_award_committed: true,
            skill_item_committed: true,
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "pet-ticket-account-service", "Blade");
        let session_id = runtime
            .current_zone_session_id()
            .expect("active shared Zone session");
        let drop = shared_gold_drop(9_201, 330, 270, None, None);
        let ticket = GroundDropClaimTicket {
            claim_id: 7,
            object_id: drop.object_id,
            drop_generation: 3,
            payload_digest: "pet-ticket-payload".to_string(),
            idempotency_key: "pet-ticket-claim".to_string(),
            session_id,
            owner_object_id: drop.owner_object_id,
            drop,
        };

        let (packets, canceled) =
            runtime.apply_shared_intelligent_creature_drop_claim(&shared_pickup_creature(), ticket);

        assert_eq!(*ground_drop_calls.lock().expect("ground drop calls"), 1);
        assert!(canceled.is_none());
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::IntelligentCreaturePickup { object_id } if *object_id == 9_201
        )));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 25 })));
        assert!(commands.lock().expect("commands").iter().any(|envelope| {
            matches!(
                &envelope.command,
                SharedAccountInventoryCommand::GroundDropClaimPickup {
                    drop,
                    claim_idempotency_key,
                } if drop.object_id == 9_201 && claim_idempotency_key == "pet-ticket-claim"
            )
        }));
    }

    #[test]
    fn intelligent_creature_shared_pickup_service_failure_cancels_without_personal_fallback() {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let ground_drop_calls = Arc::new(Mutex::new(0));
        let service = Arc::new(RecordingAccountInventoryService {
            commands,
            ground_drop_calls: ground_drop_calls.clone(),
            monster_award_calls: Arc::new(Mutex::new(0)),
            skill_item_calls: Arc::new(Mutex::new(0)),
            ground_drop_committed: false,
            monster_award_committed: true,
            skill_item_committed: true,
        }) as SharedAccountInventoryServiceHandle;
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime =
            shared_session_runtime_with_account_inventory_service(zone_state, service);
        start_new_runtime(&mut runtime, "pet-ticket-service-failure", "Blade");
        let session_id = runtime
            .current_zone_session_id()
            .expect("active shared Zone session");
        let drop = shared_gold_drop(9_202, 330, 270, None, None);
        let ticket = GroundDropClaimTicket {
            claim_id: 8,
            object_id: drop.object_id,
            drop_generation: 4,
            payload_digest: "pet-ticket-failure-payload".to_string(),
            idempotency_key: "pet-ticket-failure-claim".to_string(),
            session_id,
            owner_object_id: drop.owner_object_id,
            drop,
        };

        let (packets, canceled) =
            runtime.apply_shared_intelligent_creature_drop_claim(&shared_pickup_creature(), ticket);

        assert_eq!(*ground_drop_calls.lock().expect("ground drop calls"), 1);
        assert_eq!(canceled, Some(9_202));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::IntelligentCreaturePickup { .. } | ServerPacket::GainedGold { .. }
        )));
        assert!(runtime
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 9_202));
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
        equip_runtime_crystal_items(
            &mut runtime,
            &[
                ("Amulet", mir2_simulation::EquipmentSlot::Amulet, 5),
                (
                    "GreenPoison",
                    mir2_simulation::EquipmentSlot::BraceletRight,
                    5,
                ),
            ],
        );

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
            is_player_target: false,
            is_red_player_target: false,
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
                item_param: 0,
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
                            components,
                        } if components.len() == 2
                            && components.iter().all(|component| component.quantity == 5)
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
        equip_runtime_crystal_items(
            &mut runtime,
            &[("Amulet", mir2_simulation::EquipmentSlot::Amulet, 1)],
        );

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
            is_player_target: false,
            is_red_player_target: false,
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
                item_param: 0,
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
                            components,
                        } if components.len() == 1
                            && components[0].quantity == 1
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
            let (_, max_hp, mp) = state
                .zone_manager
                .player_vitals(&session_id)
                .expect("started Zone should expose player vitals");
            let _ = state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
                session_id: session_id.clone(),
                hp: (starting_hp - 4).max(1),
                max_hp,
                mp,
            });
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
    fn shared_zone_state_clears_stale_harvest_only_for_a_new_corpse_incarnation() {
        let mut state = SharedInProcessZoneState::new();
        let mut entity = shared_monster_entity(77);
        entity.hp = Some(12);
        entity.max_hp = Some(12);
        entity.dead = false;
        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        let harvested = ServerPacket::ObjectHarvested {
            movement: ObjectMovement {
                object_id: 77,
                position: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
            },
        };
        let died = ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 77,
                location: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
                kind: 0,
            },
        };

        // Reproduce a missed-incarnation projection: the entity is live while
        // the map layer still carries the preceding corpse's Harvest marker.
        state.apply_shared_entity_packets("0", &[harvested.clone()]);
        state.apply_shared_entity_packets("0", &[died.clone()]);
        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .harvested_entity_ids
            .contains(&77));

        // Once this corpse is harvested, a duplicate late death packet for the
        // same corpse must not reopen it.
        state.apply_shared_entity_packets("0", &[harvested]);
        state.apply_shared_entity_packets("0", &[died]);
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .harvested_entity_ids
            .contains(&77));
    }

    #[test]
    fn live_native_reconcile_clears_stale_harvest_marker() {
        let mut map = ZoneMapSnapshotLayer::default();
        map.harvested_entity_ids.insert(77);
        let mut entity = shared_monster_entity(77);
        let monster = ZoneNativeMonsterSnapshot {
            object_id: 77,
            name: entity.name.clone(),
            position: Point {
                x: entity.x,
                y: entity.y,
            },
            hp: 12,
            max_hp: 12,
            dead: false,
            disposition: Some(WorldEntityDisposition::Neutral),
            hostile_to_player: false,
        };

        reconcile_shared_entity_with_native_monster(&mut map, &mut entity, &monster);

        assert!(!map.harvested_entity_ids.contains(&77));
        assert!(!entity.dead);
        assert_eq!(entity.hp, Some(12));
    }

    #[test]
    fn shared_zone_state_resolves_harvest_target_across_crystal_front_scan() {
        let mut state = SharedInProcessZoneState::new();
        let mut corpse = shared_monster_entity(77);
        corpse.x = 330;
        corpse.y = 270;
        corpse.hp = Some(0);
        corpse.dead = true;
        let picker = shared_picker_entity(101, 328, 269);

        state.sync_map_layer(
            "0".to_string(),
            vec![corpse],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );

        assert!(state.shared_harvest_allows_action("0", &picker, MirDirection::Right));
        assert_eq!(
            state
                .shared_harvest_target_snapshot("0", &picker, MirDirection::Right)
                .map(|entity| entity.object_id),
            Some(77),
            "the gateway resolver must mirror the personal Session's 3x3 scan around the facing cell"
        );
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
    fn shared_in_process_registry_routes_melee_pvp_and_accrues_unlawful_pk() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut first, "pvp-attacker-account", "Scout");
        start_new_runtime(&mut second, "pvp-target-account", "Blade");
        first
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:100:100".to_string(),
            })
            .expect("attacker transfer should execute");
        second
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:101:100".to_string(),
            })
            .expect("target transfer should execute");
        first
            .execute(WorldCommand::ClientPacket(ClientPacket::ChangeAMode {
                mode: 5,
            }))
            .expect("all-mode change should execute");

        let first_snapshot = first.world_snapshot();
        let second_snapshot = second.world_snapshot();
        assert!(!first_snapshot.in_safe_zone && !second_snapshot.in_safe_zone);
        let attacker = first_snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .cloned()
            .expect("first session should expose its own player");
        let target = first_snapshot
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Player && entity.name == "Blade")
            .expect("first session should expose the remote player");
        assert!(
            (attacker.x - target.x)
                .abs()
                .max((attacker.y - target.y).abs())
                <= 1
        );
        let attacker_session_id = first
            .current_zone_session_id()
            .expect("attacker should have a shared-zone session");
        let target_session_id = second
            .current_zone_session_id()
            .expect("target should have a shared-zone session");
        let mut state = zone_state.lock().expect("shared zone state should lock");
        let zone = state
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .expect("shared PvP map should have a Zone runtime");
        assert_eq!(
            zone.player_chat_profile(&attacker_session_id)
                .expect("attacker profile")
                .attack_mode,
            5
        );
        assert!(
            !zone
                .player_chat_profile(&attacker_session_id)
                .expect("attacker profile")
                .in_safe_zone
        );
        assert!(
            !zone
                .player_chat_profile(&target_session_id)
                .expect("target profile")
                .in_safe_zone
        );
        let attacker_zone_position = zone
            .player_position(&attacker_session_id)
            .expect("attacker Zone position");
        let target_zone_position = zone
            .player_position(&target_session_id)
            .expect("target Zone position");
        assert!(
            (attacker_zone_position.x - target_zone_position.x)
                .abs()
                .max((attacker_zone_position.y - target_zone_position.y).abs())
                <= 1,
            "Zone positions must be adjacent: {attacker_zone_position:?} {target_zone_position:?}"
        );
        assert!(zone
            .player_vitals(&target_session_id)
            .is_some_and(|(hp, _, _)| hp > 0));
        state
            .zone_manager
            .handle(ZoneCommand::UpdatePlayerCombatStats {
                session_id: attacker_session_id,
                stats: mir2_simulation::ZonePlayerCombatStats {
                    min_dc: 500,
                    max_dc: 500,
                    accuracy: 100,
                    ..Default::default()
                },
            });
        drop(state);

        let attack_command = WorldCommand::Attack {
            object_id: target.object_id,
        };
        let resolved = first
            .prepare_zone_native_player_attack(&attack_command)
            .expect("gateway should resolve the remote player as a Zone PvP target");
        assert!(resolved.is_player_target);
        assert!(!resolved.is_red_player_target);
        let owner_packets = first
            .execute(attack_command)
            .expect("shared PvP attack should execute");
        let observer_packets = second
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 1,
            }))
            .expect("target keepalive should execute");

        assert!(
            owner_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectAttack { info } if info.object_id != target.object_id
            )),
            "attacker should receive authoritative PvP attack: {owner_packets:?}"
        );
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == target.object_id
        )));
        assert_eq!(second.world_snapshot().player_hp, Some(0));
        assert_eq!(first.world_snapshot().player_pk_points, 100);
    }

    #[test]
    fn shared_pvp_red_name_death_applies_two_item_penalty_without_penalizing_killer() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut attacker = shared_session_runtime(zone_state.clone());
        let mut target = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut attacker, "red-drop-attacker", "Hunter");
        start_new_runtime(&mut target, "red-drop-target", "Outlaw");
        attacker
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:330:280".to_string(),
            })
            .expect("attacker transfer should execute");
        target
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:331:280".to_string(),
            })
            .expect("target transfer should execute");
        for item_key in ["red-drop-a", "red-drop-b"] {
            target
                .execute(WorldCommand::Stage5Command {
                    action: "qa.giveItem".to_string(),
                    args: vec![item_key.to_string()],
                })
                .expect("PvP fixture item should be granted");
            assert!(target
                .world_snapshot()
                .inventory_items
                .iter()
                .any(|item| item.key == item_key));
        }
        attacker
            .execute(WorldCommand::ClientPacket(ClientPacket::ChangeAMode {
                mode: 5,
            }))
            .expect("all-mode change should execute");
        target.inner.apply_zone_unlawful_player_kill(300);
        target.sync_zone_snapshot();

        let before = target.world_snapshot();
        assert!(!before.in_safe_zone);
        let expected_lethal_damage = before
            .player_hp
            .expect("red-name target should expose its current HP");
        let before_item_count = before.inventory_items.len() + before.equipment_items.len();
        assert!(
            before_item_count >= 4,
            "fixture requires two droppable items"
        );
        let target_entity = attacker
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Player && entity.name == "Outlaw")
            .expect("attacker should see red-name target");
        let attacker_session_id = attacker
            .current_zone_session_id()
            .expect("attacker should have a shared-zone session");
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .handle(ZoneCommand::UpdatePlayerCombatStats {
                session_id: attacker_session_id,
                stats: mir2_simulation::ZonePlayerCombatStats {
                    min_dc: 500,
                    max_dc: 500,
                    accuracy: 100,
                    ..Default::default()
                },
            });

        attacker
            .execute(WorldCommand::Attack {
                object_id: target_entity.object_id,
            })
            .expect("red-name attack should execute");
        assert_eq!(
            target.inner.world_snapshot().player_hp,
            before.player_hp,
            "private target runtime must still be alive before it consumes the queued Zone damage"
        );
        let target_key = target.current_presence_key().expect("target presence key");
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .pending_zone_player_damages
                .get(&target_key)
                .cloned(),
            Some(vec![expected_lethal_damage])
        );
        let target_packets = target
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 1,
            }))
            .expect("target keepalive should apply death transaction");
        assert_eq!(target.inner.world_snapshot().player_hp, Some(0));
        let after = target.world_snapshot();

        assert_eq!(after.player_hp, Some(0));
        assert_eq!(
            after.inventory_items.len() + after.equipment_items.len(),
            before_item_count - 2
        );
        assert_eq!(attacker.world_snapshot().player_pk_points, 0);
        assert_eq!(
            target_packets
                .iter()
                .filter(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. }))
                .count(),
            2
        );
    }

    #[test]
    fn shared_in_process_runtime_filters_snapshots_at_crystal_object_data_range() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut first, "snapshot-range-first", "Scout");
        start_new_runtime(&mut second, "snapshot-range-second", "Blade");

        let first_key = first
            .current_presence_key()
            .expect("first runtime should have joined shared zone");
        let second_key = second
            .current_presence_key()
            .expect("second runtime should have joined shared zone");
        let first_position = Point { x: 330, y: 270 };
        let second_position = Point { x: 346, y: 286 };
        let mut first_edge_entity = shared_monster_entity(99_001);
        first_edge_entity.x = 314;
        first_edge_entity.y = 286;
        let mut first_outside_entity = shared_monster_entity(99_002);
        first_outside_entity.x = 347;
        first_outside_entity.y = 270;

        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            state.update_player_transform(&first_key, first_position.clone(), MirDirection::Down);
            state.update_player_transform(&second_key, second_position.clone(), MirDirection::Down);
            state.sync_map_layer(
                "0".to_string(),
                vec![first_edge_entity, first_outside_entity],
                BTreeSet::new(),
                vec![
                    shared_gold_drop(99_101, 314, 286, None, None),
                    shared_gold_drop(99_102, 347, 270, None, None),
                ],
                BTreeSet::new(),
            );
        }

        let first_snapshot = first.world_snapshot();
        let first_self = first_snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("first snapshot should expose self player");
        assert_eq!((first_self.x, first_self.y), (330, 270));
        assert!(first_snapshot
            .entities
            .iter()
            .any(|entity| entity.kind == WorldEntityKind::Player && entity.name == "Blade"));
        assert!(first_snapshot
            .entities
            .iter()
            .any(|entity| entity.object_id == 99_001));
        assert!(!first_snapshot
            .entities
            .iter()
            .any(|entity| entity.object_id == 99_002));
        assert!(first_snapshot
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 99_101));
        assert!(!first_snapshot
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 99_102));

        let second_snapshot = second.world_snapshot();
        let second_self = second_snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("second snapshot should expose self player");
        assert_eq!((second_self.x, second_self.y), (346, 286));
        assert!(second_snapshot
            .entities
            .iter()
            .any(|entity| entity.kind == WorldEntityKind::Player && entity.name == "Scout"));
        assert!(!second_snapshot
            .entities
            .iter()
            .any(|entity| entity.object_id == 99_001));
        assert!(second_snapshot
            .entities
            .iter()
            .any(|entity| entity.object_id == 99_002));
        assert!(!second_snapshot
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 99_101));
        assert!(second_snapshot
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 99_102));

        let shared_map = zone_state
            .lock()
            .expect("shared zone state should lock")
            .map_layer(Some("0"))
            .expect("shared map should remain available");
        assert!(shared_map.entities.contains_key(&99_001));
        assert!(shared_map.entities.contains_key(&99_002));
        assert!(shared_map.ground_drops.contains_key(&99_101));
        assert!(shared_map.ground_drops.contains_key(&99_102));
    }

    #[test]
    fn shared_in_process_registry_routes_walk_through_shared_zone() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.transfer_map("crystal:0102:3:7");
        second.transfer_map("crystal:0102:9:7");
        let start_position = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| (entity.x, entity.y))
            .expect("walk owner should have a position");

        let owner_packets = first.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        assert!(
            owner_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == start_position.0 + 1
                        && location.position.y == start_position.1
            )),
            "expected successful owner Walk ACK: {owner_packets:?}"
        );

        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 100 });
        assert!(
            observer_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectWalk { movement } if movement.direction == MirDirection::Right
            )),
            "expected observer ObjectWalk: {observer_packets:?}"
        );
    }

    #[test]
    fn shared_zone_movement_ingress_runs_while_private_runtime_is_blocked() {
        fn assert_ingress_traits<T: Clone + Send + Sync + std::fmt::Debug>() {}
        assert_ingress_traits::<SharedZoneMovementIngress>();

        let factory = SharedInProcessZoneRuntimeFactory::new();
        let zone_id = ZoneId::new("movement-ingress-blocked-runtime");
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let movement_sender = super::spawn_shared_zone_owner_with_cadence(
            &zone_id,
            zone_state.clone(),
            Duration::from_secs(60 * 60),
        );
        factory
            .zones
            .lock()
            .expect("shared zone factory should lock")
            .insert(
                zone_id.clone(),
                super::SharedInProcessZoneResources {
                    zone_state,
                    movement_sender,
                    tick_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                    autonomous_ticks_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                },
            );
        let resources = factory.resources_for_zone(&zone_id);
        let config = GatewayConfig::default();
        let mut owner = factory.create_runtime(config.clone(), &zone_id);
        let mut observer = factory.create_runtime(config, &zone_id);
        start_new_runtime_handle(&mut owner, "ingress-owner", "IngressOwner");
        start_new_runtime_handle(&mut observer, "ingress-observer", "IngressObserver");
        owner
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:330:270".to_string(),
            })
            .expect("owner should transfer to the deterministic movement fixture");
        observer
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:340:270".to_string(),
            })
            .expect("observer should transfer to the deterministic movement fixture");

        let ingress = shared_zone_movement_ingress(&owner)
            .expect("shared runtime should expose a movement ingress");
        let observer_ingress = shared_zone_movement_ingress(&observer)
            .expect("observer should expose a movement ingress");
        let owner_identity = owner
            .active_identity()
            .expect("owner should have an active identity");
        let observer_identity = observer
            .active_identity()
            .expect("observer should have an active identity");
        let owner_key = ZonePresenceKey::from_identity(&owner_identity);
        let observer_key = ZonePresenceKey::from_identity(&observer_identity);
        let (initial_position, owner_object_id) = {
            let mut zone_state = resources
                .zone_state
                .lock()
                .expect("shared zone state should lock");
            let presence = zone_state
                .players
                .get(&owner_key)
                .expect("owner presence should exist");
            let result = (
                Point {
                    x: presence.entity.x,
                    y: presence.entity.y,
                },
                presence.zone_object_id,
            );
            zone_state.take_pending_zone_packets(&observer_key);
            result
        };
        let (live_outbound_sender, mut live_outbound_receiver) = tokio::sync::mpsc::channel(8);
        let live_outbound_registration = observer_ingress
            .register_live_outbound(live_outbound_sender)
            .expect("observer live outbound should register")
            .expect("observer presence should be active");

        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let (private_snapshot_sender, private_snapshot_receiver) = sync_channel(1);
        let owner_thread = {
            let entered = entered.clone();
            let release = release.clone();
            thread::spawn(move || {
                entered.wait();
                let (released, wake) = &*release;
                let mut released = released.lock().expect("release mutex should lock");
                while !*released {
                    released = wake.wait(released).expect("release wait should resume");
                }
                drop(released);

                sync_zone_movement_transform(&mut owner)
                    .expect("pending ingress transform should sync to private runtime");
                let private_runtime = owner
                    .as_ref()
                    .as_any()
                    .downcast_ref::<SharedInProcessZoneSessionRuntime>()
                    .expect("owner should remain a shared runtime");
                private_snapshot_sender
                    .send(private_runtime.inner.world_snapshot())
                    .expect("private snapshot receiver should remain available");
            })
        };
        entered.wait();

        let started = Instant::now();
        let walk_execution = ingress
            .try_execute(ClientPacket::Walk {
                direction: MirDirection::Right,
            })
            .expect("movement ingress should execute")
            .expect("safe movement should not fall back");
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "movement ingress should not wait for the blocked private runtime"
        );
        let walk_packets = walk_execution.packets;
        let walk_position = walk_packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::UserLocation { location } => Some(location.position.clone()),
                _ => None,
            })
            .expect("owner should receive UserLocation immediately");
        assert_eq!(
            walk_position,
            Point {
                x: initial_position.x + 1,
                y: initial_position.y,
            }
        );

        let observer_walk = live_outbound_receiver
            .try_recv()
            .expect("observer should receive movement without a private runtime tick");
        assert_eq!(
            observer_walk.registration_id(),
            live_outbound_registration.registration_id()
        );
        assert!(matches!(
            observer_walk.into_packet(),
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == owner_object_id
                    && movement.position == walk_position
                    && movement.direction == MirDirection::Right
        ));
        let observer_walk_packets = resources
            .zone_state
            .lock()
            .expect("shared zone state should lock")
            .take_pending_zone_packets(&observer_key);
        assert!(!observer_walk_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectWalk { .. })));

        thread::sleep(Duration::from_millis(520));
        let started = Instant::now();
        let turn_execution = ingress
            .try_execute(ClientPacket::Turn {
                direction: MirDirection::Up,
            })
            .expect("turn ingress should execute")
            .expect("safe turn should not fall back");
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "turn ingress should not wait for the blocked private runtime"
        );
        let turn_packets = turn_execution.packets;
        assert!(
            turn_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position == walk_position && location.direction == MirDirection::Up
            )),
            "turn should return the final authoritative location: {turn_packets:?}"
        );

        let observer_turn = live_outbound_receiver
            .try_recv()
            .expect("observer should receive turn without a private runtime tick");
        assert_eq!(
            observer_turn.registration_id(),
            live_outbound_registration.registration_id()
        );
        assert!(matches!(
            observer_turn.into_packet(),
            ServerPacket::ObjectTurn { movement }
                if movement.object_id == owner_object_id
                    && movement.position == walk_position
                    && movement.direction == MirDirection::Up
        ));
        let (observer_turn_packets, pending_transform, presence_transform) = {
            let mut zone_state = resources
                .zone_state
                .lock()
                .expect("shared zone state should lock");
            let observer_packets = zone_state.take_pending_zone_packets(&observer_key);
            let pending_transform = zone_state.pending_zone_transforms.get(&owner_key).cloned();
            let presence = zone_state
                .players
                .get(&owner_key)
                .expect("owner presence should remain");
            (
                observer_packets,
                pending_transform,
                (
                    Point {
                        x: presence.entity.x,
                        y: presence.entity.y,
                    },
                    presence.entity.direction,
                ),
            )
        };
        assert!(!observer_turn_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectTurn { .. })));
        assert_eq!(
            pending_transform,
            Some((walk_position.clone(), MirDirection::Up))
        );
        assert_eq!(
            presence_transform,
            (walk_position.clone(), MirDirection::Up)
        );

        let (released, wake) = &*release;
        *released.lock().expect("release mutex should lock") = true;
        wake.notify_one();
        let private_snapshot = private_snapshot_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("private runtime should publish its synced snapshot");
        owner_thread
            .join()
            .expect("blocked private runtime thread should exit");
        let private_self = private_snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("private snapshot should contain self player");
        assert_eq!(
            (private_self.x, private_self.y),
            (walk_position.x, walk_position.y)
        );
        assert_eq!(private_self.direction, MirDirection::Up);
        assert!(!resources
            .zone_state
            .lock()
            .expect("shared zone state should lock")
            .pending_zone_transforms
            .contains_key(&owner_key));
    }

    #[test]
    fn shared_zone_movement_ingress_returns_fallback_when_queue_is_full() {
        let session_state = Arc::new(Mutex::new(super::SharedZoneMovementSessionState {
            presence_key: Some(ZonePresenceKey {
                account_id: "full-queue".to_string(),
                character_index: 0,
            }),
            presence_epoch: 1,
            ..Default::default()
        }));
        let (movement_sender, movement_receiver) = sync_channel(1);
        let ingress = SharedZoneMovementIngress {
            movement_sender: movement_sender.clone(),
            zone_state: Arc::new(Mutex::new(SharedInProcessZoneState::new())),
            session_state: session_state.clone(),
        };
        let (response_sender, _response_receiver) = sync_channel(1);
        movement_sender
            .try_send(super::SharedZoneMovementRequest {
                packet: ClientPacket::Turn {
                    direction: MirDirection::Left,
                },
                expected_presence_epoch: 1,
                session_state,
                response_sender,
            })
            .expect("first request should fill the bounded queue");

        let started = Instant::now();
        let result = ingress
            .try_execute(ClientPacket::Walk {
                direction: MirDirection::Right,
            })
            .expect("full queue should be a fallback, not an error");
        assert!(result.is_none());
        assert!(started.elapsed() < Duration::from_millis(250));
        drop(movement_receiver);
    }

    #[test]
    fn shared_zone_movement_reply_wait_is_bounded() {
        let (_response_sender, response_receiver) =
            sync_channel::<super::SharedZoneMovementReply>(1);
        let started = Instant::now();

        let error =
            super::receive_shared_zone_movement_reply(&response_receiver, Duration::from_millis(5))
                .expect_err("missing owner response should time out");

        assert_eq!(error, "shared zone movement owner timed out after 5ms");
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn stale_live_outbound_registration_cannot_remove_replacement() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let zone_id = ZoneId::new("live-outbound-epoch");
        let resources = factory.resources_for_zone(&zone_id);
        let mut runtime = factory.create_runtime(GatewayConfig::default(), &zone_id);
        start_new_runtime_handle(&mut runtime, "live-observer", "LiveObserver");
        let ingress = shared_zone_movement_ingress(&runtime)
            .expect("shared runtime should expose movement ingress");
        let key = ZonePresenceKey::from_identity(
            &runtime
                .active_identity()
                .expect("runtime should have an active identity"),
        );
        let (first_sender, mut first_receiver) = tokio::sync::mpsc::channel(4);
        let first = ingress
            .register_live_outbound(first_sender)
            .expect("first live outbound should register")
            .expect("first presence should be active");
        let (second_sender, mut second_receiver) = tokio::sync::mpsc::channel(4);
        let second = ingress
            .register_live_outbound(second_sender)
            .expect("replacement live outbound should register")
            .expect("replacement presence should be active");
        assert_ne!(first.registration_id(), second.registration_id());

        drop(first);
        resources
            .zone_state
            .lock()
            .expect("shared zone state should lock")
            .queue_zone_packets(
                key.clone(),
                vec![ServerPacket::ObjectRemove { object_id: 77 }],
            );

        assert!(first_receiver.try_recv().is_err());
        let outbound = second_receiver
            .try_recv()
            .expect("replacement socket should retain live delivery");
        assert_eq!(outbound.registration_id(), second.registration_id());
        assert!(matches!(
            outbound.into_packet(),
            ServerPacket::ObjectRemove { object_id: 77 }
        ));
        drop(second);
        assert!(!resources
            .zone_state
            .lock()
            .expect("shared zone state should lock")
            .live_zone_outbounds
            .contains_key(&key));
    }

    #[test]
    fn shared_zone_presence_epoch_advances_on_map_membership_change() {
        let key = ZonePresenceKey {
            account_id: "membership-epoch".to_string(),
            character_index: 0,
        };
        let mut state = super::SharedZoneMovementSessionState::default();

        state.activate(key.clone(), "0".to_string(), Vec::new());
        let first_epoch = state.presence_epoch;
        state.activate(key.clone(), "0".to_string(), Vec::new());
        assert_eq!(state.presence_epoch, first_epoch);

        state.activate(key, "1".to_string(), Vec::new());
        assert_eq!(state.presence_epoch, first_epoch + 1);
        state.deactivate();
        assert_eq!(state.presence_epoch, first_epoch + 2);
    }

    #[test]
    fn shared_zone_movement_ingress_returns_fallback_near_map_transfer() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let zone_id = ZoneId::new("movement-ingress-map-transfer");
        let resources = factory.resources_for_zone(&zone_id);
        let mut runtime = factory.create_runtime(GatewayConfig::default(), &zone_id);
        start_new_runtime_handle(&mut runtime, "ingress-transfer", "TransferScout");
        runtime
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:307:264".to_string(),
            })
            .expect("test transfer should execute");
        let ingress = shared_zone_movement_ingress(&runtime)
            .expect("shared runtime should expose a movement ingress");
        let identity = runtime
            .active_identity()
            .expect("runtime should retain active identity");
        let key = ZonePresenceKey::from_identity(&identity);
        let before = {
            let zone_state = resources
                .zone_state
                .lock()
                .expect("shared zone state should lock");
            let presence = zone_state
                .players
                .get(&key)
                .expect("transfer presence should exist");
            (
                presence.entity.x,
                presence.entity.y,
                presence.entity.direction,
            )
        };

        let result = ingress
            .try_execute(ClientPacket::Walk {
                direction: MirDirection::Right,
            })
            .expect("near-transfer ingress check should execute");
        assert!(result.is_none());
        let after = {
            let zone_state = resources
                .zone_state
                .lock()
                .expect("shared zone state should lock");
            let presence = zone_state
                .players
                .get(&key)
                .expect("transfer presence should remain");
            (
                presence.entity.x,
                presence.entity.y,
                presence.entity.direction,
            )
        };
        assert_eq!(after, before, "fallback must not mutate shared Zone state");
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
    fn shared_gateway_turn_commits_allowed_crystal_map_coordinate_transfer() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(Arc::clone(&zone_state));
        start_new_runtime(&mut runtime, "mapcoord-turn", "MapCoordTurn");

        let mut checkpoint = runtime
            .inner
            .active_character_checkpoint()
            .expect("started character checkpoint");
        checkpoint.pk_points = 200;
        runtime
            .inner
            .restore_active_character_checkpoint(&checkpoint)
            .expect("PK gate fixture should restore");
        runtime
            .execute(WorldCommand::TransferMap {
                key: "crystal:3:861:686".to_string(),
            })
            .expect("fixture should enter the Crystal source coordinate");

        let packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Turn {
                direction: MirDirection::Left,
            }))
            .expect("shared Turn should execute");

        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::MapInformation { info } if info.file_name == "D1801"
            )),
            "allowed shared Turn must commit the authoritative E1 transfer: {packets:?}"
        );
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position == Point { x: 128, y: 171 }
                    && location.direction == MirDirection::Left
        )));
        let snapshot = runtime.world_snapshot();
        assert_eq!(snapshot.map_file_name.as_deref(), Some("D1801"));
        let player = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("transferred player snapshot");
        assert_eq!((player.x, player.y), (128, 171));
        assert_eq!(player.direction, MirDirection::Left);
    }

    #[test]
    fn shared_in_process_registry_routes_monster_attack_through_zone_native_combat() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let map_file_name = first
            .world_snapshot()
            .map_file_name
            .clone()
            .expect("first session should be in a map");
        assert!(
            !map_file_name.is_empty(),
            "first session should report a non-empty map"
        );

        // Replace the personal starter fixture with the real shared map layer,
        // then seed a bounded test monster that remains owned by the shared Zone.
        first.transfer_map(&format!("crystal:{map_file_name}:330:270"));
        first.stage5_command(
            "event.spawn",
            vec!["RakingCat0".to_string(), "1".to_string()],
        );
        let monster = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.name == "RakingCat0"
                    && entity.disposition == WorldEntityDisposition::Hostile
                    && !entity.dead
            })
            .expect("shared map should expose a live hostile monster");
        let player = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("first session should expose its shared player");
        if (player.x - monster.x).abs() > 1 || (player.y - monster.y).abs() > 1 {
            let direction = if player.y > monster.y {
                MirDirection::Up
            } else if player.y < monster.y {
                MirDirection::Down
            } else if player.x > monster.x {
                MirDirection::Left
            } else {
                MirDirection::Right
            };
            first.handle_packet(ClientPacket::Walk { direction });
        }
        let player = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("first session should remain visible after approaching");
        assert!(
            (player.x - monster.x).abs() <= 1 && (player.y - monster.y).abs() <= 1,
            "event fixture should spawn adjacent: player=({}, {}), monster=({}, {})",
            player.x,
            player.y,
            monster.x,
            monster.y
        );
        let owner_packets = first.attack(monster.object_id);
        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 100 });

        assert!(owner_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id != monster.object_id
        )));
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id != monster.object_id
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
    fn shared_harvest_syncs_private_player_to_authoritative_zone_transform() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "harvest-transform", "Harvester");
        runtime
            .execute(WorldCommand::TransferMap {
                key: "crystal:0:285:630".to_string(),
            })
            .expect("harvest fixture transfer should execute");

        let identity = runtime
            .inner
            .active_identity()
            .expect("started session should expose an identity");
        let key = ZonePresenceKey::from_identity(&identity);
        let mut corpse = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .filter(|entity| entity.kind == WorldEntityKind::Monster && entity.name == "Deer")
            .min_by_key(|entity| (entity.x - 285).abs().max((entity.y - 630).abs()))
            .expect("harvest fixture should expose a Crystal Deer");
        corpse.object_id = 9_500_002;
        corpse.ai = Some(2);
        corpse.x = 288;
        corpse.y = 636;
        corpse.direction = MirDirection::DownLeft;
        corpse.hp = Some(0);
        corpse.dead = true;
        let mut unrelated_shared_monster = shared_monster_entity(9_500_003);
        unrelated_shared_monster.name = "RakingCat".to_string();
        unrelated_shared_monster.x = corpse.x + 8;
        unrelated_shared_monster.y = corpse.y + 8;
        let authoritative_position = Point {
            x: corpse.x + 1,
            y: corpse.y,
        };
        assert!(runtime.inner.apply_shared_entity_snapshot(&corpse));
        runtime.inner.force_authoritative_player_transform(
            authoritative_position.clone(),
            MirDirection::Left,
        );

        {
            let mut shared = zone_state.lock().expect("shared zone state should lock");
            let session_id = shared
                .zone_sessions
                .get(&key)
                .cloned()
                .expect("harvest fixture should have joined the shared Zone");
            let outbounds = shared
                .zone_manager
                .handle(ZoneCommand::SyncPlayerTransform {
                    session_id,
                    position: authoritative_position.clone(),
                    direction: MirDirection::Left,
                });
            let _ = shared.dispatch_zone_outbounds(outbounds, Some(&key));
            shared.sync_map_layer(
                "0".to_string(),
                vec![corpse.clone(), unrelated_shared_monster.clone()],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        }

        // Low-latency Zone movement can advance independently of the private
        // SimulationSession. Reproduce that split by leaving the private
        // player at its preceding coordinate before issuing Harvest.
        runtime.inner.force_authoritative_player_transform(
            Point {
                x: authoritative_position.x + 8,
                y: authoritative_position.y,
            },
            MirDirection::Left,
        );

        let unrelated_private_ai_before = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .filter(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.object_id != corpse.object_id
            })
            .map(|entity| {
                (
                    entity.object_id,
                    (entity.x, entity.y, entity.hp, entity.dead),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Harvest {
                direction: MirDirection::Left,
            }))
            .expect("shared harvest should execute");

        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position == authoritative_position
                        && location.direction == MirDirection::Left
            )),
            "Harvest must execute from the authoritative Zone transform: {packets:?}"
        );
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectHarvest { .. })),
            "the preflight gate must use the authoritative transform and permit the adjacent shared corpse: {packets:?}"
        );
        assert!(
            !runtime
                .inner
                .world_snapshot()
                .entities
                .iter()
                .any(|entity| entity.object_id == unrelated_shared_monster.object_id),
            "Harvest must not materialize and tick unrelated shared monsters"
        );
        let unrelated_private_ai_after = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .filter(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.object_id != corpse.object_id
            })
            .map(|entity| {
                (
                    entity.object_id,
                    (entity.x, entity.y, entity.hp, entity.dead),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (object_id, before) in &unrelated_private_ai_before {
            assert_eq!(
                unrelated_private_ai_after.get(object_id),
                Some(before),
                "Harvest must not advance unrelated private monster AI {object_id}"
            );
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectWalk { movement }
                    | ServerPacket::ObjectRun { movement }
                    | ServerPacket::ObjectTurn { movement }
                    if movement.object_id == *object_id
            )));
        }

        let mut all_packets = packets;
        for _ in 0..6 {
            if all_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectHarvested { .. }))
            {
                break;
            }
            runtime
                .execute(WorldCommand::Tick)
                .expect("an interleaved client world tick should preserve the shared corpse");
            let pass = runtime
                .execute(WorldCommand::ClientPacket(ClientPacket::Harvest {
                    direction: MirDirection::Left,
                }))
                .expect("subsequent shared harvest pass should execute");
            assert!(
                pass.iter()
                    .any(|packet| matches!(packet, ServerPacket::ObjectHarvest { .. })),
                "every accepted skinning pass must retain the shared corpse target: {pass:?}"
            );
            all_packets.extend(pass);
        }
        assert!(
            all_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectHarvested { .. })),
            "the shared corpse must finish its multi-pass harvest lifecycle: {all_packets:?}"
        );

        let incarnation_packets = vec![
            ServerPacket::ObjectRevived {
                info: ObjectRevivedInfo {
                    object_id: corpse.object_id,
                    effect: true,
                },
            },
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: corpse.object_id,
                    location: Point {
                        x: corpse.x,
                        y: corpse.y,
                    },
                    direction: corpse.direction,
                    kind: 0,
                },
            },
        ];
        {
            let mut shared = zone_state.lock().expect("shared zone state should lock");
            shared.apply_shared_entity_packets("0", &incarnation_packets);
            shared.queue_zone_packets(key, incarnation_packets.clone());
        }
        let pending = runtime.apply_pending_zone_packets();
        assert!(pending.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRevived { info } if info.object_id == corpse.object_id
        )));
        let next_incarnation = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Harvest {
                direction: MirDirection::Left,
            }))
            .expect("the respawned shared corpse should execute its first harvest pass");
        assert!(
            next_incarnation
                .iter()
                .any(|packet| matches!(packet, ServerPacket::ObjectHarvest { .. })),
            "explicit Zone revive must reset the private compatibility harvest state: {next_incarnation:?}"
        );
    }

    #[test]
    fn shared_melee_direction_uses_authoritative_zone_transform() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "melee-direction", "Fighter");
        let target = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && !entity.dead
                    && entity.hp.is_some_and(|hp| hp > 0)
            })
            .expect("starter scene should expose a live monster");
        let key = runtime
            .current_presence_key()
            .expect("started runtime should have a shared presence");
        let authoritative_position = Point {
            x: target.x + 1,
            y: target.y,
        };
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .update_player_transform(&key, authoritative_position.clone(), MirDirection::Left);
        runtime.inner.force_authoritative_player_transform(
            Point {
                x: target.x,
                y: target.y + 8,
            },
            MirDirection::Up,
        );

        let prepared = runtime
            .prepare_zone_native_player_attack(&WorldCommand::Attack {
                object_id: target.object_id,
            })
            .expect("shared monster attack should resolve");

        assert_eq!(prepared.direction, MirDirection::Left);
    }

    #[test]
    fn shared_in_process_registry_qa_apply_native_state_syncs_zone_transform() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);
        session.transfer_map("crystal:0:334:263");

        let payload = r#"{
            "character": {
                "name": "NativeScout",
                "level": 6,
                "class": "Warrior",
                "gender": "Male"
            },
            "mapFileName": "0",
            "mapTitle": "BichonProvince",
            "position": { "x": 335, "y": 266 },
            "direction": "UpRight",
            "hp": 51,
            "maxHp": 51,
            "mp": 32,
            "maxMp": 32,
            "experience": 435,
            "maxExperience": 900,
            "gold": 3457,
            "credit": 0,
            "inventoryItemsJson": [],
            "beltItemsJson": [],
            "storageItemsJson": [],
            "equipmentItemsJson": []
        }"#;

        let packets = session.stage5_command("qa.applyNativeState", vec![payload.to_string()]);
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position.x == 335 && location.position.y == 266
        )));
        let snapshot = session.world_snapshot();
        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("self player should remain visible after QA native-state apply");

        assert_eq!(snapshot.map_file_name.as_deref(), Some("0"));
        assert_eq!((self_entity.x, self_entity.y), (335, 266));
        assert_eq!(self_entity.direction, MirDirection::UpRight);
    }

    #[test]
    fn shared_in_process_registry_syncs_visible_paid_sailor_round_trip_into_zone() {
        let config = GatewayConfig::default().with_platinum_176_profile();
        {
            let mut account_store = config
                .account_store
                .lock()
                .expect("paid sailor account fixture should lock");
            let account = account_store
                .accounts
                .get_mut("demo")
                .expect("demo account fixture should exist");
            account
                .characters
                .iter_mut()
                .find(|character| character.index == 0)
                .expect("demo character fixture should exist")
                .level = 14;
            let save = account
                .saves
                .get_mut(&0)
                .expect("demo character save fixture should exist");
            save.character.level = 14;
            save.gold = 5_000;
        }
        let registry = ZoneRegistry::in_process();
        let mut session = GatewaySession::new_with_zone_registry(config, &registry);
        start_demo_character(&mut session);
        let _ = session.transfer_map("crystal:0:251:676");

        let _ = session.interact(9);
        assert!(session
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog
                .links
                .iter()
                .any(|link| link.target.eq_ignore_ascii_case("@brdmove"))));
        let outbound_packets = session.select_npc_dialog_target("@brdmove");
        assert!(outbound_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MapInformation { info } if info.file_name == "5"
        )));
        let outbound = session.world_snapshot();
        let outbound_player = outbound
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("outbound player should remain in the shared Zone snapshot");
        assert_eq!(outbound.map_file_name.as_deref(), Some("5"));
        assert_eq!((outbound_player.x, outbound_player.y), (124, 353));
        assert_eq!(outbound.gold, 3_000);

        let _ = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Left,
        });
        let _ = session.interact(1169);
        assert!(session
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog
                .links
                .iter()
                .any(|link| link.target.eq_ignore_ascii_case("@brdmove"))));
        let return_packets = session.select_npc_dialog_target("@brdmove");
        assert!(return_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MapInformation { info } if info.file_name == "0"
        )));
        let returned = session.world_snapshot();
        let returned_player = returned
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("returned player should remain in the shared Zone snapshot");
        assert_eq!(returned.map_file_name.as_deref(), Some("0"));
        assert_eq!((returned_player.x, returned_player.y), (253, 673));
        assert_eq!(returned.gold, 1_000);
    }

    #[test]
    fn shared_in_process_registry_rejects_walk_outside_bichon_collision_bounds() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);
        session.transfer_map("crystal:0:288:634");

        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::DownLeft,
        });

        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == 288 && location.position.y == 634
            )),
            "walking outside the Starter collision bounds should return the authoritative origin: {packets:?}"
        );
    }

    #[test]
    fn shared_in_process_registry_rejects_run_outside_bichon_collision_bounds() {
        let registry = ZoneRegistry::in_process();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut session);
        session.transfer_map("crystal:0:288:634");

        let packets = session.handle_packet(ClientPacket::Run {
            direction: MirDirection::DownLeft,
        });

        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == 288 && location.position.y == 634
            )),
            "running outside the Starter collision bounds should return the authoritative origin: {packets:?}"
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
    fn shared_in_process_crystal_world_runtime_does_not_apply_starter_demo_gate_transfer() {
        let registry = ZoneRegistry::in_process();
        let mut session = GatewaySession::new_with_zone_registry(
            GatewayConfig::default().with_crystal_world_runtime(),
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
            "Full Crystal world runtime should not use the starter demo same-map gate: {packets:?}"
        );
        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == 339 && location.position.y == 270
            )),
            "walking right from 338,270 should remain normal movement in full Crystal world runtime: {packets:?}"
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
    fn shared_zone_owner_cadence_consumes_queued_player_movement_without_session_tick() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.transfer_map("crystal:0102:3:7");
        second.transfer_map("crystal:0102:9:7");
        let owner_ingress = first
            .zone_movement_ingress()
            .expect("owner should expose shared Zone ingress");
        let observer_ingress = second
            .zone_movement_ingress()
            .expect("observer should expose shared Zone ingress");
        let (owner_sender, mut owner_receiver) = tokio::sync::mpsc::channel(16);
        let (observer_sender, mut observer_receiver) = tokio::sync::mpsc::channel(16);
        let _owner_registration = owner_ingress
            .register_live_outbound(owner_sender)
            .expect("owner live outbound should register")
            .expect("owner presence should be active");
        let _observer_registration = observer_ingress
            .register_live_outbound(observer_sender)
            .expect("observer live outbound should register")
            .expect("observer presence should be active");

        let first_packets = first.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        assert!(first_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position.x == 4 && location.position.y == 7
        )));
        while owner_receiver.try_recv().is_ok() {}
        while observer_receiver.try_recv().is_ok() {}

        let queued_packets = first.handle_packet(ClientPacket::Run {
            direction: MirDirection::Right,
        });
        assert!(
            !queued_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
            "run sent before the Crystal movement cadence should queue: {queued_packets:?}"
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut owner_location = None;
        let mut observer_movement = None;
        while Instant::now() < deadline && (owner_location.is_none() || observer_movement.is_none())
        {
            while let Ok(outbound) = owner_receiver.try_recv() {
                if let ServerPacket::UserLocation { location } = outbound.into_packet() {
                    owner_location = Some(location.position);
                }
            }
            while let Ok(outbound) = observer_receiver.try_recv() {
                if let ServerPacket::ObjectRun { movement } = outbound.into_packet() {
                    observer_movement = Some(movement);
                }
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(owner_location, Some(Point { x: 6, y: 7 }));
        assert!(matches!(
            observer_movement,
            Some(ObjectMovement {
                position: Point { x: 6, y: 7 },
                direction: MirDirection::Right,
                ..
            })
        ));
    }

    #[test]
    fn shared_zone_cadence_is_not_multiplied_by_personal_session_ticks() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "cadence-second", "CadenceB");

        first
            .execute(WorldCommand::Tick)
            .expect("first personal tick should execute");
        second
            .execute(WorldCommand::Tick)
            .expect("second personal tick should execute");
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .zone_cadence_tick_count,
            0,
            "personal Session ticks must not drive the global Zone clock"
        );

        super::run_shared_zone_cadence_tick(&zone_state, shared_gateway_now_ms())
            .expect("the Zone owner cadence should tick");
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .zone_cadence_tick_count,
            1,
            "one Zone cadence event should advance the global Zone exactly once"
        );
    }

    #[test]
    fn shared_gateway_tick_does_not_run_private_monster_authority() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_demo_runtime(&mut runtime);

        let before = runtime.inner.world_snapshot();
        let player_object_id = before
            .player_object_id
            .expect("started session should expose its local player id");
        let target = before
            .entities
            .iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.disposition == WorldEntityDisposition::Hostile
                    && entity.hp.is_some_and(|hp| hp > 0)
            })
            .cloned()
            .expect("starter scene should expose a live hostile monster");
        let adjacent = Point {
            x: target.x.saturating_sub(1),
            y: target.y,
        };
        let session_id = runtime
            .current_zone_session_id()
            .expect("started session should have a shared-Zone session");
        let _ = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position: adjacent.clone(),
                direction: MirDirection::Right,
            },
            false,
        );
        runtime
            .inner
            .force_authoritative_player_transform(adjacent, MirDirection::Right);
        let before = runtime.inner.world_snapshot();
        let before_hp = before.player_hp;
        let before_tick = before.tick;
        let before_target = before
            .entities
            .iter()
            .find(|entity| entity.object_id == target.object_id)
            .cloned()
            .expect("private compatibility world should retain the target");

        let mut packets = Vec::new();
        for _ in 0..64 {
            packets.extend(
                runtime
                    .execute(WorldCommand::Tick)
                    .expect("shared Gateway personal tick should execute"),
            );
        }

        let after = runtime.inner.world_snapshot();
        let after_target = after
            .entities
            .iter()
            .find(|entity| entity.object_id == target.object_id)
            .expect("passive personal ticks must not retire the Zone-owned target");
        assert_eq!(after.tick, before_tick.saturating_add(64));
        assert_eq!(after.player_hp, before_hp);
        assert_eq!(
            (
                after_target.x,
                after_target.y,
                after_target.direction,
                after_target.hp,
                after_target.dead,
            ),
            (
                before_target.x,
                before_target.y,
                before_target.direction,
                before_target.hp,
                before_target.dead,
            ),
            "the personal compatibility clock must not move or damage a Zone-owned monster"
        );
        assert!(
            !packets.iter().any(|packet| match packet {
                ServerPacket::ObjectWalk { movement }
                | ServerPacket::ObjectRun { movement }
                | ServerPacket::ObjectTurn { movement } => {
                    movement.object_id == target.object_id
                }
                ServerPacket::ObjectAttack { info } => info.object_id == target.object_id,
                ServerPacket::ObjectDied { info } => {
                    info.object_id == target.object_id || info.object_id == player_object_id
                }
                ServerPacket::Death { .. } => true,
                _ => false,
            }),
            "shared Gateway ticks must not emit private monster movement/combat/death: {packets:?}"
        );
    }

    #[test]
    fn shared_in_process_registry_post_movement_grace_yields_world_tick() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_demo_runtime(&mut runtime);

        let tick_before_grace = runtime.inner.world_snapshot().tick;
        runtime
            .movement_ingress
            .session_state
            .lock()
            .expect("movement session state should lock")
            .recent_zone_player_movement_until_ms = u64::MAX;

        let grace_packets = runtime
            .execute(WorldCommand::Tick)
            .expect("post-movement grace tick should execute");

        assert!(
            grace_packets.is_empty(),
            "post-movement grace should yield without heavy world packets: {grace_packets:?}"
        );
        assert_eq!(
            runtime.inner.world_snapshot().tick,
            tick_before_grace,
            "post-movement grace should leave the local runtime tick free for follow-up input"
        );

        runtime
            .movement_ingress
            .session_state
            .lock()
            .expect("movement session state should lock")
            .recent_zone_player_movement_until_ms = 0;
        runtime
            .execute(WorldCommand::Tick)
            .expect("normal world tick should execute after grace");
        assert!(
            runtime.inner.world_snapshot().tick > tick_before_grace,
            "normal world tick should resume once the post-movement input window closes"
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
    fn shared_in_process_registry_syncs_equipped_mount_before_three_tile_run() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let native_state = serde_json::json!({
            "character": {
                "name": "Demo",
                "level": 22,
                "class": "Warrior",
                "gender": "Male"
            },
            "mapFileName": "0",
            "mapTitle": "BichonProvince",
            "position": { "x": 330, "y": 270 },
            "direction": "Down",
            "hp": 18,
            "maxHp": 258,
            "mp": 14,
            "maxMp": 88,
            "experience": 0,
            "maxExperience": 100,
            "gold": 0,
            "credit": 0,
            "inventoryItemsJson": [],
            "beltItemsJson": [],
            "storageItemsJson": [],
            "equipmentItemsJson": []
        });
        first.stage5_command("qa.applyNativeState", vec![native_state.to_string()]);
        // MeatStore (0102) is authoritatively `NoMount` in Server.MirDB.
        // Exercise mounted three-tile movement on a known walkable, mount-allowed
        // Bichon fixture instead of relying on the former incomplete map metadata.
        first.transfer_map("crystal:0:330:270");
        second.transfer_map("crystal:0:340:270");

        first.stage5_command("qa.giveItem", vec!["crystal-item-769".to_string()]);
        let mount_unique_id = first
            .world_snapshot()
            .inventory_items
            .iter()
            .find(|item| item.key == "crystal-item-769")
            .map(|item| item.unique_id)
            .expect("QA RedTiger should be granted");
        let equip_packets = first.handle_packet(ClientPacket::EquipItem {
            grid: MirGridType::Inventory,
            unique_id: mount_unique_id,
            to: 13,
        });
        assert!(equip_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::EquipItem {
                success: true,
                to: 13,
                ..
            }
        )));

        let ride_packets = first.handle_packet(ClientPacket::UseItem {
            unique_id: 13,
            grid: MirGridType::Equipment,
        });
        assert!(ride_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                mount_type: 5,
                riding_mount: true,
                ..
            }
        )));
        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 102 });
        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                mount_type: 5,
                riding_mount: true,
                ..
            }
        )));

        let start = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| Point {
                x: entity.x,
                y: entity.y,
            })
            .expect("mounted owner should have a position");
        first.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        thread::sleep(Duration::from_millis(650));
        let run_packets = first.handle_packet(ClientPacket::Run {
            direction: MirDirection::Right,
        });
        assert!(
            run_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::UserLocation { location }
                    if location.position.x == start.x + 4 && location.position.y == start.y
            )),
            "mounted Walk + Run should move one plus three tiles: {run_packets:?}"
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
    fn shared_in_process_registry_routes_gm_chat_commands_to_personal_session() {
        let (mut runtime, mut observer) = started_shared_zone_sessions();
        let player_id = runtime
            .world_snapshot()
            .player_object_id
            .expect("started session should expose player object id");

        let packets = runtime.handle_packet(ClientPacket::Chat {
            message: "@DIE".to_string(),
            linked_items: Vec::new(),
        });

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Death { .. })));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == player_id
        )));
        assert_eq!(runtime.world_snapshot().player_hp, Some(0));
        assert!(observer
            .handle_packet(ClientPacket::KeepAlive { time: 105 })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectDied { .. })));

        let revive_packets = runtime.handle_packet(ClientPacket::TownRevive);
        assert!(revive_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Revived)));
        assert!(revive_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRevived { info } if info.object_id == player_id
        )));
        assert!(runtime.world_snapshot().player_hp.is_some_and(|hp| hp > 0));
    }

    #[test]
    fn shared_in_process_town_revive_preserves_private_death_until_zone_sync() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "private-death-revive", "Survivor");
        let player_id = runtime
            .inner
            .world_snapshot()
            .player_object_id
            .expect("started session should expose its local player id");
        let bind_position = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| Point {
                x: entity.x,
                y: entity.y,
            })
            .expect("started session should expose its bind position");
        let bind_map_file_name = runtime
            .inner
            .world_snapshot()
            .map_file_name
            .expect("started session should expose its bind map");

        runtime
            .execute(WorldCommand::TransferMap {
                key: "crystal:2:406:453".to_string(),
            })
            .expect("field transfer should execute");
        assert_eq!(
            runtime.inner.world_snapshot().map_file_name.as_deref(),
            Some("2")
        );
        let session_id = runtime
            .current_zone_session_id()
            .expect("started session should have a shared-zone session");

        // Reproduce the production split-authority edge: a private world tick
        // has killed the player and emitted the Crystal death markers while the
        // shared Zone still retains its preceding positive HP value.
        let mut death_packets = runtime
            .inner
            .execute(WorldCommand::ClientPacket(ClientPacket::Chat {
                message: "@DIE".to_string(),
                linked_items: Vec::new(),
            }))
            .expect("private death fixture should execute");
        assert!(death_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Death { .. })));
        runtime.filter_stale_owner_dead_entity_packets(&mut death_packets);
        assert_eq!(runtime.inner.world_snapshot().player_hp, Some(0));
        assert!(zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .player_vitals(&session_id)
            .is_some_and(|(hp, _, _)| hp > 0));

        // A following personal tick must keep the acknowledged death instead
        // of restoring the Zone's stale positive HP and emitting Death again.
        let tick_packets = runtime
            .execute(WorldCommand::Tick)
            .expect("dead player tick should execute");
        assert!(!tick_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Death { .. })));
        assert_eq!(runtime.inner.world_snapshot().player_hp, Some(0));

        // The self ObjectDied marker can be pruned by an intervening shared
        // snapshot even though the browser-visible private death remains. The
        // TownRevive command itself must still preserve that presented death
        // across the pre-command Zone vitals reconciliation.
        runtime.owner_dead_entity_ids.clear();

        let revive_packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
            .expect("town revive should execute");
        assert!(revive_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Revived)));
        assert!(revive_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRevived { info } if info.object_id == player_id
        )));
        assert!(revive_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MapInformation { info } if info.file_name == bind_map_file_name
        )));
        assert!(runtime.world_snapshot().player_hp.is_some_and(|hp| hp > 0));
        assert_eq!(
            runtime.world_snapshot().map_file_name.as_deref(),
            Some(bind_map_file_name.as_str())
        );
        assert_eq!(
            zone_state
                .lock()
                .expect("shared zone state should lock")
                .zone_manager
                .player_transform(&session_id)
                .map(|(position, _)| position),
            Some(bind_position)
        );
    }

    #[test]
    fn shared_gateway_dead_potion_requires_town_revive_and_logout_saves_zone_authority() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let config = GatewayConfig::default()
            .with_crystal_world_runtime()
            .with_platinum_176_profile();
        let account_store = config.account_store.clone();
        let mut runtime = shared_session_runtime(zone_state.clone());
        runtime.inner = InProcessWorldRuntime::new(config);
        let account_id = "gateway-dead-potion-save";
        start_new_runtime(&mut runtime, account_id, "PotionSentinel");
        let identity = runtime
            .inner
            .active_identity()
            .expect("started runtime should expose its authenticated identity");

        let snapshot = runtime.inner.world_snapshot();
        let potion_unique_id = snapshot
            .inventory_items
            .iter()
            .find(|item| item.name == "(HP)DrugSmall")
            .map(|item| item.unique_id)
            .expect("Platinum starter inventory should contain one HP drug");
        let start_position = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .map(|entity| Point {
                x: entity.x,
                y: entity.y,
            })
            .expect("started player should expose a position");

        runtime
            .execute(WorldCommand::ApplyHandoffTransform {
                position: start_position.clone(),
                direction: MirDirection::Down,
                hp: Some(10),
                mp: None,
            })
            .expect("fixture should lower authoritative HP");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
                unique_id: potion_unique_id,
                grid: MirGridType::Inventory,
            }))
            .expect("normal HP drug should queue its timed recovery");
        runtime
            .execute(WorldCommand::ApplyHandoffTransform {
                position: start_position,
                direction: MirDirection::Down,
                hp: Some(0),
                mp: None,
            })
            .expect("fixture should mark both private and Zone authorities dead");

        let tick_packets = runtime
            .execute(WorldCommand::Tick)
            .expect("dead-player Gateway tick should execute");
        assert_eq!(runtime.inner.world_snapshot().player_hp, Some(0));
        assert_eq!(runtime.world_snapshot().player_hp, Some(0));
        assert!(!tick_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Revived | ServerPacket::ObjectRevived { .. }
        )));

        let revive_packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
            .expect("explicit TownRevive should execute through the Gateway");
        assert!(revive_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Revived)));
        assert!(runtime.world_snapshot().player_hp.is_some_and(|hp| hp > 0));

        // Recreate the exact split that exists after low-latency Zone movement
        // and combat: the shared Zone has the latest transform and vitals while
        // the private compatibility runtime still holds older values. Logout
        // must reconcile those authorities before the personal save is written.
        let key = runtime
            .current_presence_key()
            .expect("revived player should retain a Zone presence");
        let session_id = runtime
            .current_zone_session_id()
            .expect("revived player should retain a Zone session");
        let private_before_logout = runtime
            .inner
            .active_character_checkpoint()
            .expect("active character should expose a checkpoint");
        let desired_position = Point {
            x: private_before_logout.position.x.saturating_add(7),
            y: private_before_logout.position.y.saturating_add(5),
        };
        let saved_hp = 17;
        let authoritative_position = {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            let (_, max_hp, mp) = state
                .zone_manager
                .player_vitals(&session_id)
                .expect("Zone should expose player vitals");
            state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
                session_id: session_id.clone(),
                hp: saved_hp,
                max_hp,
                mp,
            });
            let outbounds = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
                session_id: session_id.clone(),
                position: desired_position,
                direction: MirDirection::Left,
            });
            let _ = state.dispatch_zone_outbounds(outbounds, Some(&key));
            state
                .zone_manager
                .player_transform(&session_id)
                .map(|(position, _)| position)
                .expect("Zone should retain the authoritative transform")
        };
        assert_ne!(private_before_logout.hp, saved_hp);
        assert_ne!(private_before_logout.position, authoritative_position);

        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .expect("logout should reconcile and persist Zone authority");
        let store = account_store
            .lock()
            .expect("test account store should lock after logout");
        let save = store
            .accounts
            .get(&identity.account_id)
            .and_then(|account| account.saves.get(&identity.character_index))
            .expect("logout should persist the active character");
        assert_eq!(save.hp, saved_hp);
        assert_eq!(save.position, authoritative_position);
        assert_eq!(save.direction, MirDirection::Left);
    }

    #[test]
    fn shared_in_process_registry_syncs_stage5_event_spawn_to_zone() {
        let (mut runtime, _) = started_shared_zone_sessions();
        let before_ids = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .filter(|entity| entity.kind == WorldEntityKind::Monster)
            .map(|entity| entity.object_id)
            .collect::<BTreeSet<_>>();

        let packets = runtime.stage5_command(
            "event.spawn",
            vec!["RakingCat0".to_string(), "1".to_string()],
        );

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.name == "RakingCat0" && !before_ids.contains(&info.object_id)
        )));
        let snapshot = runtime.world_snapshot();
        let spawned = snapshot
            .entities
            .iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.name == "RakingCat0"
                    && !before_ids.contains(&entity.object_id)
            })
            .expect("stage5 event.spawn should be visible through the shared zone snapshot");
        assert_eq!(spawned.disposition, WorldEntityDisposition::Hostile);
        assert!(spawned.hp.is_some_and(|hp| hp > 0));
    }

    #[test]
    fn shared_in_process_runtime_broadcasts_delayed_player_tick_damage() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut first);
        start_new_runtime(&mut second, "second-delayed", "Blade");
        let mut target = shared_monster_entity(260_501);
        target.name = "RakingCat0".to_string();
        target.disposition = WorldEntityDisposition::Hostile;
        target.x = 331;
        target.y = 270;
        target.hp = Some(32);
        target.max_hp = Some(32);
        target.sprite = Some(mir2_simulation::WorldEntitySpriteSnapshot {
            body_library: "Monster/007".to_string(),
            direction_stride: 4,
            frame_base_offset: 0,
            frame_count: 4,
            hair_library: None,
            weapon_library: None,
            weapon_library_secondary: None,
            weapon_frame_offset: None,
            mount_library: None,
            mount_frame_offset: None,
            alt_body_library: None,
            alt_hair_library: None,
            alt_weapon_library: None,
            alt_weapon_library_secondary: None,
            alt_weapon_frame_offset: None,
            alt_frame_base_offset: None,
        });
        let attacker_position = Point { x: 330, y: 270 };
        let first_key = first
            .current_presence_key()
            .expect("first runtime should have joined the shared Zone");
        {
            let mut state = zone_state.lock().expect("shared zone state should lock");
            state.update_player_transform(
                &first_key,
                attacker_position.clone(),
                MirDirection::Right,
            );
            state.sync_map_layer(
                "0".to_string(),
                vec![target.clone()],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        }
        first
            .inner
            .force_authoritative_player_transform(attacker_position, MirDirection::Right);
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

        super::run_shared_zone_cadence_tick(&zone_state, shared_gateway_now_ms())
            .expect("Zone cadence should resolve delayed damage");
        let owner_tick_packets = first
            .execute(WorldCommand::Tick)
            .expect("owner personal tick should drain Zone results");
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
    fn shared_in_process_runtime_level_one_field_melee_resolves_damage_on_tick() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "field-combat", "FieldBlade");
        let mut target = shared_monster_entity(260_500);
        target.name = "RakingCat0".to_string();
        target.disposition = WorldEntityDisposition::Hostile;
        target.x = 331;
        target.y = 270;
        target.hp = Some(32);
        target.max_hp = Some(32);
        target.sprite = Some(mir2_simulation::WorldEntitySpriteSnapshot {
            body_library: "Monster/007".to_string(),
            direction_stride: 4,
            frame_base_offset: 0,
            frame_count: 4,
            hair_library: None,
            weapon_library: None,
            weapon_library_secondary: None,
            weapon_frame_offset: None,
            mount_library: None,
            mount_frame_offset: None,
            alt_body_library: None,
            alt_hair_library: None,
            alt_weapon_library: None,
            alt_weapon_library_secondary: None,
            alt_weapon_frame_offset: None,
            alt_frame_base_offset: None,
        });
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .sync_map_layer(
                "0".to_string(),
                vec![target.clone()],
                BTreeSet::new(),
                Vec::new(),
                BTreeSet::new(),
            );
        runtime.inner.force_authoritative_player_transform(
            Point {
                x: target.x.saturating_sub(1),
                y: target.y,
            },
            MirDirection::Right,
        );
        runtime.sync_zone_snapshot();

        let launch_packets = runtime
            .execute(WorldCommand::Attack {
                object_id: target.object_id,
            })
            .expect("attack should execute");
        assert!(launch_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id != target.object_id
        )));

        super::run_shared_zone_cadence_tick(&zone_state, shared_gateway_now_ms())
            .expect("Zone cadence should resolve the field melee hit");
        let tick_packets = runtime
            .execute(WorldCommand::Tick)
            .expect("personal tick should drain the resolved field melee hit");
        assert!(tick_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == target.object_id
        )));
        assert!(tick_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::DamageIndicator { object_id, .. } if *object_id == target.object_id
        )));
    }

    #[test]
    fn shared_in_process_runtime_routes_range_attack_through_shared_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut first = shared_session_runtime(zone_state.clone());
        let mut second = shared_session_runtime(zone_state.clone());
        start_new_runtime_with_class(&mut first, "first-range", "Arrow", MirClass::Archer);
        equip_runtime_crystal_items_for_class(
            &mut first,
            MirClass::Archer,
            &[("WoodenBow", mir2_simulation::EquipmentSlot::Weapon, 1)],
        );
        start_new_runtime(&mut second, "second-range", "Blade");
        let target = first
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.disposition == WorldEntityDisposition::Hostile
                    && entity.hp.is_some_and(|hp| hp > 1)
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

        super::run_shared_zone_cadence_tick(&zone_state, shared_gateway_now_ms())
            .expect("Zone cadence should resolve the range attack");
        let owner_tick_packets = first
            .execute(WorldCommand::Tick)
            .expect("owner personal tick should drain Zone results");
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
        let mut second = shared_session_runtime(zone_state.clone());
        start_new_runtime_with_class(&mut first, "first-range-window", "Arrow", MirClass::Archer);
        equip_runtime_crystal_items_for_class(
            &mut first,
            MirClass::Archer,
            &[("WoodenBow", mir2_simulation::EquipmentSlot::Weapon, 1)],
        );
        start_new_runtime(&mut second, "second-range-window", "Blade");
        let target = first
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && entity.disposition == WorldEntityDisposition::Hostile
                    && entity.hp.is_some_and(|hp| hp > 1)
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
        super::run_shared_zone_cadence_tick(&zone_state, shared_gateway_now_ms())
            .expect("Zone cadence should resolve the first range attack");
        let _ = first.apply_pending_zone_packets();
        let _ = second.apply_pending_zone_packets();

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
    fn gateway_refreshes_trusted_range_admission_and_rejects_unauthorized_sessions() {
        for (case, account_id, name, class, weapon) in [
            (
                "warrior",
                "range-war",
                "DenyWar",
                MirClass::Warrior,
                "WoodenSword",
            ),
            (
                "archer-without-class-weapon",
                "range-bow",
                "DenyBow",
                MirClass::Archer,
                "WoodenSword",
            ),
        ] {
            let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
            let mut runtime = shared_session_runtime(zone_state);
            start_new_runtime_with_class(&mut runtime, account_id, name, class);
            equip_runtime_crystal_items_for_class(
                &mut runtime,
                class,
                &[(weapon, mir2_simulation::EquipmentSlot::Weapon, 1)],
            );
            let (target, attacker_position) = prepare_gateway_range_fixture(&mut runtime, 3);
            let packets = runtime
                .execute(gateway_range_attack_command(&target, attacker_position))
                .expect("unauthorized range intent should be handled");

            assert!(
                packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
                "{case} should receive an owner correction: {packets:?}"
            );
            assert!(
                !packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::RangeAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
                )),
                "{case} must not launch a shared range attack: {packets:?}"
            );
        }
    }

    #[test]
    fn gateway_unauthorized_materialized_range_is_atomic() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime_with_class(
            &mut runtime,
            "range-atomic-warrior",
            "AtomicWarrior",
            MirClass::Warrior,
        );
        let session_id = runtime
            .current_zone_session_id()
            .expect("runtime should own a Zone session");
        assert!(runtime
            .sync_authoritative_zone_combat_state(&session_id)
            .is_some());
        let mut target = shared_monster_entity(260_901);
        target.name = "RakingCat0".to_string();
        target.x = 331;
        target.y = 270;
        target.disposition = WorldEntityDisposition::Hostile;
        let monster = zone_monster_spawn_from_shared_entity(&target, 0)
            .expect("shared monster should materialize");
        let before = zone_state
            .lock()
            .expect("Zone state should lock")
            .zone_manager
            .checkpoint_bytes()
            .expect("Zone checkpoint");

        let packets = runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
            object_id: target.object_id,
            is_player_target: false,
            is_red_player_target: false,
            direction: MirDirection::Right,
            level: 0,
            damage: 999,
            monster: Some(monster.clone()),
            kind: ZoneNativePlayerAttackKind::Range {
                target: Point {
                    x: target.x,
                    y: target.y,
                },
                spell: Spell::None,
                attack_type: 0,
            },
        });

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { .. }
                | ServerPacket::RangeAttack { .. }
                | ServerPacket::ObjectRangeAttack { .. }
        )));
        let state = zone_state.lock().expect("Zone state should lock");
        assert!(state
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map("0"))
            .iter()
            .all(|monster| monster.object_id != target.object_id));
        assert_eq!(
            state
                .zone_manager
                .checkpoint_bytes()
                .expect("Zone checkpoint"),
            before
        );
    }

    #[test]
    fn gateway_legal_archer_cannot_materialize_neutral_range_target() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime_with_class(
            &mut runtime,
            "range-neutral-archer",
            "NeutralArrow",
            MirClass::Archer,
        );
        equip_runtime_crystal_items_for_class(
            &mut runtime,
            MirClass::Archer,
            &[("WoodenBow", mir2_simulation::EquipmentSlot::Weapon, 1)],
        );
        let session_id = runtime
            .current_zone_session_id()
            .expect("runtime should own a Zone session");
        assert!(runtime
            .sync_authoritative_zone_combat_state(&session_id)
            .is_some());
        runtime
            .inner
            .force_authoritative_player_transform(Point { x: 330, y: 270 }, MirDirection::Right);
        let target = shared_monster_entity(260_902);
        let monster = zone_monster_spawn_from_shared_entity(&target, 0)
            .expect("neutral shared monster should be representable");
        assert_eq!(monster.ai, 1);
        let before = zone_state
            .lock()
            .expect("Zone state should lock")
            .zone_manager
            .checkpoint_bytes()
            .expect("Zone checkpoint");

        let packets = runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
            object_id: target.object_id,
            is_player_target: false,
            is_red_player_target: false,
            direction: MirDirection::Right,
            level: 0,
            damage: 999,
            monster: Some(monster),
            kind: ZoneNativePlayerAttackKind::Range {
                target: Point {
                    x: target.x,
                    y: target.y,
                },
                spell: Spell::None,
                attack_type: 0,
            },
        });

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { .. }
                | ServerPacket::RangeAttack { .. }
                | ServerPacket::ObjectRangeAttack { .. }
        )));
        let state = zone_state.lock().expect("Zone state should lock");
        assert!(state
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map("0"))
            .iter()
            .all(|monster| monster.object_id != target.object_id));
        assert_eq!(
            state
                .zone_manager
                .checkpoint_bytes()
                .expect("Zone checkpoint"),
            before
        );
    }

    #[test]
    fn gateway_melee_materializes_neutral_harvestable_deer() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "neutral-deer-melee", "DeerBlade");
        let session_id = runtime
            .current_zone_session_id()
            .expect("runtime should own a Zone session");
        assert!(runtime
            .sync_authoritative_zone_combat_state(&session_id)
            .is_some());
        runtime
            .inner
            .force_authoritative_player_transform(Point { x: 330, y: 270 }, MirDirection::UpLeft);
        let mut target = shared_monster_entity(260_904);
        target.ai = Some(2);
        target.x = 329;
        target.y = 269;
        let monster = zone_monster_spawn_from_shared_entity(&target, 0)
            .expect("neutral Deer should be representable");
        assert_eq!(monster.disposition, Some(WorldEntityDisposition::Neutral));
        assert!(monster.is_authoritatively_melee_attackable_by_player());
        assert!(!monster.is_authoritatively_hostile_to_player());

        let packets = runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
            object_id: target.object_id,
            is_player_target: false,
            is_red_player_target: false,
            direction: MirDirection::UpLeft,
            level: 0,
            damage: 999,
            monster: Some(monster),
            kind: ZoneNativePlayerAttackKind::Melee {
                spell: Spell::None as u8,
                attack_type: 0,
            },
        });

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
        assert!(zone_state
            .lock()
            .expect("Zone state should lock")
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map("0"))
            .iter()
            .any(|monster| monster.object_id == target.object_id));
    }

    #[test]
    fn gateway_prefilter_uses_explicit_disposition_for_friendly_and_hostile_ai0() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(
            &mut runtime,
            "explicit-disposition-warrior",
            "DispositionBlade",
        );
        let session_id = runtime
            .current_zone_session_id()
            .expect("runtime should own a Zone session");
        assert!(runtime
            .sync_authoritative_zone_combat_state(&session_id)
            .is_some());
        runtime
            .inner
            .force_authoritative_player_transform(Point { x: 330, y: 270 }, MirDirection::UpLeft);
        let mut target = shared_monster_entity(260_903);
        target.ai = Some(0);
        target.disposition = WorldEntityDisposition::Friendly;
        let monster = zone_monster_spawn_from_shared_entity(&target, 0)
            .expect("friendly shared monster should be representable");
        assert_eq!(monster.ai, 0);
        assert_eq!(monster.disposition, Some(WorldEntityDisposition::Friendly));
        let before = zone_state
            .lock()
            .expect("Zone state should lock")
            .zone_manager
            .checkpoint_bytes()
            .expect("Zone checkpoint");

        let melee_packets = runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
            object_id: target.object_id,
            is_player_target: false,
            is_red_player_target: false,
            direction: MirDirection::UpLeft,
            level: 0,
            damage: 999,
            monster: Some(monster.clone()),
            kind: ZoneNativePlayerAttackKind::Melee {
                spell: Spell::None as u8,
                attack_type: 0,
            },
        });

        assert!(matches!(
            melee_packets.as_slice(),
            [ServerPacket::UserLocation { .. }]
        ));
        assert!(!melee_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMonster { .. }
                | ServerPacket::ObjectAttack { .. }
                | ServerPacket::DamageIndicator { .. }
        )));

        let range_packets = runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
            object_id: target.object_id,
            is_player_target: false,
            is_red_player_target: false,
            direction: MirDirection::UpLeft,
            level: 0,
            damage: 999,
            monster: Some(monster.clone()),
            kind: ZoneNativePlayerAttackKind::Range {
                target: Point {
                    x: target.x,
                    y: target.y,
                },
                spell: Spell::None,
                attack_type: 0,
            },
        });
        assert!(matches!(
            range_packets.as_slice(),
            [ServerPacket::UserLocation { .. }]
        ));

        let mut incomplete = monster.clone();
        incomplete.object_id = target.object_id + 10;
        incomplete.disposition = None;
        let incomplete_packets =
            runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
                object_id: incomplete.object_id,
                is_player_target: false,
                is_red_player_target: false,
                direction: MirDirection::UpLeft,
                level: 0,
                damage: 999,
                monster: Some(incomplete),
                kind: ZoneNativePlayerAttackKind::Melee {
                    spell: Spell::None as u8,
                    attack_type: 0,
                },
            });
        assert!(matches!(
            incomplete_packets.as_slice(),
            [ServerPacket::UserLocation { .. }]
        ));
        {
            let state = zone_state.lock().expect("Zone state should lock");
            assert!(state
                .zone_manager
                .native_monster_snapshots(&ZoneKey::for_map("0"))
                .iter()
                .all(|monster| monster.object_id != target.object_id));
            assert_eq!(
                state
                    .zone_manager
                    .checkpoint_bytes()
                    .expect("Zone checkpoint"),
                before
            );
        }

        let hostile_object_id = target.object_id + 1;
        let mut hostile = target;
        hostile.object_id = hostile_object_id;
        hostile.disposition = WorldEntityDisposition::Hostile;
        let hostile_spawn = zone_monster_spawn_from_shared_entity(&hostile, 0)
            .expect("hostile AI0 monster should be representable");
        assert_eq!(hostile_spawn.ai, 0);
        assert_eq!(
            hostile_spawn.disposition,
            Some(WorldEntityDisposition::Hostile)
        );
        let accepted = runtime.execute_zone_native_player_attack(ZoneNativePlayerAttack {
            object_id: hostile_object_id,
            is_player_target: false,
            is_red_player_target: false,
            direction: MirDirection::UpLeft,
            level: 0,
            damage: 999,
            monster: Some(hostile_spawn),
            kind: ZoneNativePlayerAttackKind::Melee {
                spell: Spell::None as u8,
                attack_type: 0,
            },
        });
        assert!(accepted
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
    }

    #[test]
    fn gateway_refreshes_changed_combat_state_before_the_next_range_intent() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut attacker = shared_session_runtime(zone_state.clone());
        let mut observer = shared_session_runtime(zone_state);
        start_new_runtime_with_class(
            &mut attacker,
            "range-stale-a",
            "FreshArrow",
            MirClass::Archer,
        );
        equip_runtime_crystal_items_for_class(
            &mut attacker,
            MirClass::Archer,
            &[("WoodenBow", mir2_simulation::EquipmentSlot::Weapon, 1)],
        );
        start_new_runtime(&mut observer, "range-stale-o", "Watcher");
        let (target, attacker_position) = prepare_gateway_range_fixture(&mut attacker, 3);
        observer.sync_zone_snapshot();

        let session_id = attacker
            .current_zone_session_id()
            .expect("attacker should have a Zone session");
        assert!(attacker
            .sync_authoritative_zone_combat_state(&session_id)
            .is_some());
        equip_runtime_crystal_items_for_class(
            &mut attacker,
            MirClass::Archer,
            &[("WoodenSword", mir2_simulation::EquipmentSlot::Weapon, 1)],
        );
        // Seed the Zone with the previously valid record to prove that the
        // Gateway refreshes instead of trusting stale admission.
        let _ = attacker.dispatch_zone_player_command(
            ZoneCommand::sync_player_combat_state(
                session_id,
                MirClass::Archer,
                true,
                false,
                true,
                false,
                false,
                false,
            ),
            false,
        );

        let packets = attacker
            .execute(gateway_range_attack_command(&target, attacker_position))
            .expect("changed-state range intent should be handled");
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::RangeAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
        )));
        let observer_packets = observer
            .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 1,
            }))
            .expect("observer should drain pending packets");
        assert!(!observer_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectRangeAttack { .. })));
    }

    #[test]
    fn gateway_refreshes_mount_attack_capability_before_each_melee_intent() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "mount-melee", "MountedBlade");
        equip_runtime_crystal_items_for_class(
            &mut runtime,
            MirClass::Warrior,
            &[("RedTiger", mir2_simulation::EquipmentSlot::Mount, 1)],
        );
        let ride_packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
                unique_id: 13,
                grid: MirGridType::Equipment,
            }))
            .expect("mount ride should execute");
        assert!(ride_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MountUpdate {
                riding_mount: true,
                ..
            }
        )));
        let (target, attacker_position) = prepare_gateway_range_fixture(&mut runtime, 1);
        let mounted = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("self snapshot");
        assert_eq!(mounted.riding_mount, Some(true));
        assert_eq!(mounted.can_mount_attack, Some(false));

        // Seed a previously permissive trusted state. The attack boundary must
        // replace it from the authenticated personal snapshot before admission.
        let session_id = runtime
            .current_zone_session_id()
            .expect("runtime should have a Zone session");
        let _ = runtime.dispatch_zone_player_command(
            ZoneCommand::sync_player_combat_state(
                session_id,
                MirClass::Warrior,
                false,
                false,
                true,
                false,
                false,
                false,
            ),
            false,
        );
        runtime
            .inner
            .force_authoritative_player_transform(attacker_position, MirDirection::Right);

        let packets = runtime
            .execute(WorldCommand::Attack {
                object_id: target.object_id,
            })
            .expect("mounted melee intent should be handled");
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
    }

    #[test]
    fn gateway_shared_range_uses_crystal_nine_tile_boundary() {
        for (distance, accepted) in [(9, true), (10, false)] {
            let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
            let mut runtime = shared_session_runtime(zone_state);
            start_new_runtime_with_class(
                &mut runtime,
                &format!("range-b-{distance}"),
                &format!("Range{distance}"),
                MirClass::Archer,
            );
            equip_runtime_crystal_items_for_class(
                &mut runtime,
                MirClass::Archer,
                &[("WoodenBow", mir2_simulation::EquipmentSlot::Weapon, 1)],
            );
            let (target, attacker_position) = prepare_gateway_range_fixture(&mut runtime, distance);
            let packets = runtime
                .execute(gateway_range_attack_command(&target, attacker_position))
                .expect("boundary range intent should execute");
            let launched = packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectRangeAttack { info }
                        if info.target_id == target.object_id
                )
            });
            assert_eq!(launched, accepted, "distance={distance}: {packets:?}");
            if !accepted {
                assert!(packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
            }
        }
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
                entity.kind == WorldEntityKind::Monster
                    && entity.disposition == WorldEntityDisposition::Hostile
                    && entity.hp.is_some_and(|hp| hp > 1)
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
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_demo_runtime(&mut runtime);
        let mut monster = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live monster");
        let authoritative_hp = zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map("0"))
            .into_iter()
            .find(|native| native.object_id == monster.object_id)
            .map(|native| native.hp)
            .expect("starter monster should have Zone-native vitals");
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
        assert_eq!(local_hp, Some(authoritative_hp));
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
            ServerPacket::ObjectMonster { info }
                if info.object_id == movement.object_id && info.ai == 1
        )));
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

        assert!(!observer_packets
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
    fn shared_in_process_registry_keeps_npc_quest_icons_personal_per_session() {
        let registry = ZoneRegistry::in_process();
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut second = GatewaySession::new_with_zone_registry(config, &registry);
        start_new_character(&mut first, "quest-icon-first", "IconBladeA");
        start_new_character(&mut second, "quest-icon-second", "IconBladeB");

        first.transfer_map("crystal:0:283:606");
        let opened = first.handle_packet(ClientPacket::CallNpc {
            object_id: 3,
            key: "@Main".to_owned(),
        });
        assert!(opened
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectChat { object_id: 3, .. })));
        let accepted = first.select_npc_dialog_target("@quest:accept:1");
        assert!(accepted.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1,
                taken: true,
                completed: true,
                ..
            }
        )));
        first.transfer_map("crystal:0:288:616");
        second.transfer_map("crystal:0:288:616");

        let first_snapshot = first.world_snapshot();
        let second_snapshot = second.world_snapshot();
        let icon = |snapshot: &mir2_simulation::WorldSnapshot, object_id: u32| {
            snapshot
                .entities
                .iter()
                .find(|entity| entity.kind == WorldEntityKind::Npc && entity.object_id == object_id)
                .and_then(|entity| entity.quest_icon)
        };

        assert_eq!(icon(&first_snapshot, 3), None);
        assert_eq!(icon(&first_snapshot, 4), Some(3));
        assert_eq!(
            icon(&second_snapshot, 3),
            Some(2),
            "the second player's available q1 marker must survive the first player's progress"
        );
        assert_eq!(
            icon(&second_snapshot, 4),
            None,
            "the first player's ready marker must not leak through shared Zone state"
        );
    }

    #[test]
    fn shared_npc_interact_uses_authoritative_zone_transform_after_long_movement() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "npc-zone-transform", "Walker");
        let shared_npc = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::Npc)
            .expect("shared runtime should expose an NPC");

        runtime
            .execute(WorldCommand::TransferMap {
                key: format!(
                    "crystal:0:{}:{}",
                    shared_npc.x.saturating_sub(1),
                    shared_npc.y
                ),
            })
            .expect("test transfer should place the Zone player beside the NPC");

        // Reproduce the real low-latency split: Zone movement has reached the
        // NPC, while the personal compatibility Session still carries an old
        // hunting-field transform. NPC distance checks must use the Zone
        // position at the command boundary just like Harvest and pickup do.
        runtime.inner.force_authoritative_player_transform(
            Point {
                x: shared_npc.x.saturating_add(8),
                y: shared_npc.y.saturating_add(8),
            },
            MirDirection::Down,
        );

        let packets = runtime
            .execute(WorldCommand::Interact {
                object_id: shared_npc.object_id,
            })
            .expect("shared NPC interaction should execute");
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == shared_npc.object_id
        )));
        assert_eq!(
            runtime
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
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == shared_guide.object_id
        )));
        let dialog_snapshot = second.world_snapshot();
        assert_eq!(
            dialog_snapshot
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(shared_guide.object_id)
        );
        second.select_npc_dialog_target("@AcceptQuest:1001");
        let snapshot = second.world_snapshot();
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
    fn shared_quest_packet_uses_authoritative_zone_transform_after_callnpc() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "quest-zone-transform", "Blade");
        let shared_guide = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == mir2_simulation::WorldEntityKind::Npc
                    && entity.quest_ids.contains(&1001)
            })
            .expect("default session should expose a shared quest NPC");

        runtime
            .execute(WorldCommand::TransferMap {
                key: format!(
                    "crystal:0:{}:{}",
                    shared_guide.x.saturating_sub(1),
                    shared_guide.y
                ),
            })
            .expect("test transfer should place the Zone player beside the guide");

        let packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::CallNpc {
                object_id: shared_guide.object_id,
                key: "@Main".to_string(),
            }))
            .expect("shared guide CallNpc should execute");
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == shared_guide.object_id
        )));
        let dialog_snapshot = runtime.world_snapshot();
        assert_eq!(
            dialog_snapshot
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(shared_guide.object_id)
        );
        // A delayed private-session transform must not invalidate the dialog
        // after CallNpc has admitted it from the authoritative shared Zone.
        // FinishQuest uses the same command-boundary synchronization path.
        runtime.inner.force_authoritative_player_transform(
            Point {
                x: shared_guide.x.saturating_add(8),
                y: shared_guide.y.saturating_add(8),
            },
            MirDirection::Down,
        );
        let accepted = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::AcceptQuest {
                npc_index: shared_guide.object_id,
                quest_index: 1001,
            }))
            .expect("shared guide AcceptQuest should execute");
        assert!(accepted.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                taken: true,
                ..
            }
        )));
        let snapshot = runtime.world_snapshot();
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
        let gold_before = first.world_snapshot().gold;

        let owner_packets = first.handle_packet(ClientPacket::DropGold { amount: 100 });
        assert_eq!(first.world_snapshot().gold, gold_before - 100);
        let gold_location = owner_packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::ObjectGold { info } => Some(info.location.clone()),
                _ => None,
            })
            .expect("owner should receive ObjectGold for dropped gold");
        let observer_packets = second.handle_packet(ClientPacket::KeepAlive { time: 106 });
        first.handle_packet(ClientPacket::KeepAlive { time: 107 });

        assert!(observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectGold { info } if info.gold == 100 && info.location == gold_location
        )));
        assert_eq!(
            first.world_snapshot().gold,
            gold_before - 100,
            "a keepalive must not reapply or reset a committed gold projection"
        );
    }

    #[test]
    fn shared_in_process_registry_allocates_unique_ids_for_concurrent_player_gold_drops() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let second_state = serde_json::json!({
            "character": {
                "name": "Blade",
                "level": 1,
                "class": "Warrior",
                "gender": "Male"
            },
            "mapFileName": "0",
            "mapTitle": "BichonProvince",
            "position": { "x": 345, "y": 280 },
            "direction": "Down",
            "hp": 100,
            "maxHp": 100,
            "mp": 100,
            "maxMp": 100,
            "experience": 0,
            "maxExperience": 100,
            "gold": 1000,
            "credit": 0,
            "inventoryItemsJson": [],
            "beltItemsJson": [],
            "storageItemsJson": [],
            "equipmentItemsJson": []
        });
        second.stage5_command("qa.applyNativeState", vec![second_state.to_string()]);

        let first_packets = first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let second_packets = second.handle_packet(ClientPacket::DropGold { amount: 100 });
        let drop_from = |packets: &[ServerPacket]| {
            packets.iter().rev().find_map(|packet| match packet {
                ServerPacket::ObjectGold { info } => {
                    Some((info.object_id, info.location.x, info.location.y))
                }
                _ => None,
            })
        };
        let first_drop = drop_from(&first_packets).expect("first gold drop should spawn");
        let second_drop = drop_from(&second_packets).expect("second gold drop should spawn");

        assert_ne!(
            first_drop.0, second_drop.0,
            "session-local drop ids must be remapped by the shared Zone owner"
        );
        let shared_drop_ids = first
            .world_snapshot()
            .ground_drops
            .into_iter()
            .map(|drop| drop.object_id)
            .collect::<BTreeSet<_>>();
        assert!(shared_drop_ids.contains(&first_drop.0));
        assert!(shared_drop_ids.contains(&second_drop.0));

        first.transfer_map(&format!("crystal:0:{}:{}", first_drop.1, first_drop.2));
        second.transfer_map(&format!("crystal:0:{}:{}", second_drop.1, second_drop.2));
        assert!(first
            .pick_up(first_drop.0)
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        assert!(second
            .pick_up(second_drop.0)
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        assert!(first.world_snapshot().ground_drops.is_empty());
        assert!(second.world_snapshot().ground_drops.is_empty());
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
        let gained_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::GainedItem { .. }))
            .expect("shared pickup should report the gained item");
        let remove_index = packets
            .iter()
            .position(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectRemove { object_id }
                        if *object_id == shared_drop.object_id
                )
            })
            .expect("shared pickup should remove the claimed ground object");
        assert!(
            gained_index < remove_index,
            "Crystal sends GainedItem before ObjectRemove"
        );
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
    fn shared_pickup_zone_path_is_handled_even_when_it_emits_no_packets() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_new_runtime(&mut runtime, "zone-empty-pickup", "Scout");

        let result = runtime.pick_up_shared_drop(Some(u32::MAX));

        assert_eq!(result, Some(Vec::new()));
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
    fn shared_in_process_runtime_claims_packet_spawned_shared_drop_through_zone() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "packet-drop-claim", "Scout");
        let self_entity = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose self player");
        runtime.apply_shared_entity_packets_to_current_map(&[ServerPacket::ObjectItem {
            info: ObjectItemInfo {
                object_id: 8810,
                name: "Venison".to_string(),
                name_colour_argb: -1,
                location: Point {
                    x: self_entity.x,
                    y: self_entity.y,
                },
                image: 5,
                grade: 0,
            },
        }]);

        assert!(runtime
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 8810));
        assert!(zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .is_some_and(|zone| !zone.has_ground_drop(8810)));

        let packets = runtime
            .execute(WorldCommand::PickUp { object_id: 8810 })
            .expect("packet-spawned shared pickup should execute");

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(!runtime
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 8810));
        assert!(zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .is_some_and(|zone| !zone.has_ground_drop(8810)));
    }

    #[test]
    fn shared_in_process_runtime_pickup_uses_zone_authoritative_position_for_packet_drop() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state.clone());
        start_new_runtime(&mut runtime, "packet-drop-zone-position", "Scout");
        let stale_inner_self = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose inner self player");
        let authoritative_position = Point {
            x: stale_inner_self.x + 3,
            y: stale_inner_self.y,
        };
        let drop_position = Point {
            x: authoritative_position.x + 1,
            y: authoritative_position.y,
        };
        let key = runtime
            .current_presence_key()
            .expect("runtime should have joined shared zone");
        zone_state
            .lock()
            .expect("shared zone state should lock")
            .update_player_transform(&key, authoritative_position.clone(), MirDirection::Right);

        let visible_self = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("runtime should expose visible self player");
        assert_eq!(
            (visible_self.x, visible_self.y),
            (authoritative_position.x, authoritative_position.y)
        );
        assert_eq!(
            (stale_inner_self.x, stale_inner_self.y),
            (drop_position.x - 4, drop_position.y)
        );

        runtime.apply_shared_entity_packets_to_current_map(&[ServerPacket::ObjectItem {
            info: ObjectItemInfo {
                object_id: 8811,
                name: "Venison".to_string(),
                name_colour_argb: -1,
                location: drop_position,
                image: 5,
                grade: 0,
            },
        }]);

        let packets = runtime
            .execute(WorldCommand::PickUp { object_id: 8811 })
            .expect("packet-spawned shared pickup should execute");

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(!runtime
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == 8811));
        assert!(zone_state
            .lock()
            .expect("shared zone state should lock")
            .zone_manager
            .zone(&ZoneKey::for_map("0"))
            .is_some_and(|zone| !zone.has_ground_drop(8811)));
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
    fn shared_in_process_runtime_expires_shared_drop_on_zone_cadence() {
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

        super::run_shared_zone_cadence_tick(&zone_state, shared_gateway_now_ms())
            .expect("shared Zone cadence should expire the drop");

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
    fn shared_in_process_registry_commits_trade_through_economy_boundary() {
        let bootstraps = Arc::new(Mutex::new(0));
        let trades = Arc::new(Mutex::new(Vec::new()));
        let service = Arc::new(RecordingTradeSettlementService {
            bootstraps: Arc::clone(&bootstraps),
            trades: Arc::clone(&trades),
        }) as SharedAccountInventoryServiceHandle;
        let factory =
            Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service))
                as SharedZoneRuntimeFactory;
        let registry = ZoneRegistry::new(ZoneId::primary(), factory);
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut second = GatewaySession::new_with_zone_registry(config, &registry);
        start_demo_character(&mut first);
        start_new_character(&mut second, "trade-ledger-second", "LedgerBob");

        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        second.handle_packet(ClientPacket::TradeGold { amount: 40 });
        second.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert_eq!(*bootstraps.lock().expect("bootstrap count should lock"), 2);
        let trades = trades.lock().expect("recorded trades should lock");
        assert_eq!(trades.len(), 1);
        let (second_offer, first_offer) = &trades[0];
        assert_eq!(second_offer.account_id, "trade-ledger-second");
        assert_eq!(second_offer.gold, 0);
        assert_eq!(first_offer.account_id, "demo");
        assert_eq!(first_offer.gold, 30);
    }

    #[test]
    fn unfenced_initial_trade_is_deferred_and_rolls_back_exactly_once() {
        let unresolved = Arc::new(Mutex::new(true));
        let calls = Arc::new(Mutex::new(0));
        let service = Arc::new(UnknownThenRejectedTradeSettlementService {
            unresolved,
            calls: Arc::clone(&calls),
        }) as SharedAccountInventoryServiceHandle;
        let factory =
            Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service));
        let registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::clone(&factory) as SharedZoneRuntimeFactory,
        );
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut second = GatewaySession::new_with_zone_registry(config, &registry);
        start_demo_character(&mut first);
        start_new_character(&mut second, "trade-deferred-second", "DeferredBob");

        let first_starting_gold = first.world_snapshot().gold;
        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        let second_packets = second.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert_eq!(*calls.lock().expect("settlement calls should lock"), 1);
        assert!(second_packets
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 92 });
        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        {
            let resources = factory.resources_for_zone(&ZoneId::primary());
            let state = resources.zone_state.lock().expect("zone state should lock");
            assert!(state.unresolved_trade_settlements.is_empty());
            assert!(state.pending_trade_deliveries.is_empty());
            assert!(state.pending_trade_rollbacks.is_empty());
        }

        let repeat = first.handle_packet(ClientPacket::KeepAlive { time: 93 });
        assert!(repeat
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedGold { .. })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert_eq!(*calls.lock().expect("settlement calls should lock"), 1);
    }
    #[test]
    fn shared_trade_repeat_request_cannot_replace_debited_unmatched_offer() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let starting_gold = first.world_snapshot().gold;
        let red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        let original = first
            .world_snapshot()
            .stage5_systems
            .trade
            .expect("completed unmatched offer");
        assert_eq!(first.world_snapshot().gold, starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        assert!(first.handle_packet(ClientPacket::TradeRequest).is_empty());
        let preserved = first
            .world_snapshot()
            .stage5_systems
            .trade
            .expect("original unmatched offer remains");
        assert_eq!(preserved.settlement_nonce, original.settlement_nonce);
        assert_eq!(preserved.offered_gold, 30);
        assert!(preserved.completed);

        second.handle_packet(ClientPacket::TradeCancel);
        first.handle_packet(ClientPacket::KeepAlive { time: 31 });
        assert_eq!(first.world_snapshot().gold, starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    #[test]
    fn unmatched_offerer_logout_rolls_back_before_persisting_character() {
        let (mut first, _second) = started_shared_zone_sessions();
        let starting_gold = first.world_snapshot().gold;
        let red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        let logout = first.handle_packet(ClientPacket::LogOut);
        assert!(logout
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(logout.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert!(logout
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));

        first.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert_eq!(first.world_snapshot().gold, starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
        assert!(first.world_snapshot().stage5_systems.trade.is_none());
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

    #[test]
    fn shared_runtime_npc_teleport_routes_protocol_atomically_and_persists_transform() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let map = ZoneMapMetadata {
            map_index: 1,
            file_name: "0".to_string(),
            title: "BichonProvince".to_string(),
            mini_map: 1,
            big_map: 101,
            lights: 2,
            map_dark_light: 0,
            music: 0,
            weather: 0,
        };
        let configured_zone = ZoneRuntime::new_with_collision_and_npc_teleport_config(
            ZoneKey::for_map("0"),
            ZoneCollision::unbounded(),
            ZoneNpcTeleportConfig {
                enabled: true,
                cost: 3_000,
                maps: BTreeMap::from([("0".to_string(), map)]),
                destinations: vec![ZoneNpcTeleportDestination {
                    map_file_name: "0".to_string(),
                    object_id: 900,
                }],
            },
        );
        assert!(zone_state
            .lock()
            .unwrap()
            .zone_manager
            .install_empty_zone(configured_zone));

        let mut runtime = shared_session_runtime(Arc::clone(&zone_state));
        start_demo_runtime(&mut runtime);
        let mut checkpoint = runtime.inner.active_character_checkpoint().unwrap();
        checkpoint.gold = 9_000;
        runtime
            .inner
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let session_id = runtime.current_zone_session_id().unwrap();
        zone_state
            .lock()
            .unwrap()
            .zone_manager
            .handle(ZoneCommand::SyncSharedObjects {
                session_id,
                packets: vec![ServerPacket::ObjectNpc {
                    info: NpcInfo {
                        object_id: 900,
                        name: "Teleport Guide".to_string(),
                        name_colour_argb: -1,
                        image: 12,
                        colour_argb: -1,
                        location: Point { x: 40, y: 40 },
                        direction: MirDirection::Down,
                        quest_ids: Vec::new(),
                    },
                }],
                include_owner: false,
                now_ms: 1,
            });

        let packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TeleportToNpc {
                object_id: 900,
            }))
            .unwrap();

        assert!(matches!(
            packets.first(),
            Some(ServerPacket::LoseGold { gold: 3_000 })
        ));
        assert!(matches!(
            packets.get(1),
            Some(ServerPacket::MapChanged {
                map_index: 1,
                file_name,
                location,
                ..
            }) if file_name == "0" && location == &Point { x: 40, y: 41 }
        ));
        assert!(matches!(
            packets.get(2),
            Some(ServerPacket::UserLocation { location })
                if location.position == Point { x: 40, y: 41 }
        ));
        assert_eq!(runtime.world_snapshot().gold, 6_000);
        let self_entity = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        assert_eq!((self_entity.x, self_entity.y), (40, 41));

        let second_attempt = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TeleportToNpc {
                object_id: 900,
            }))
            .unwrap();
        assert!(second_attempt.is_empty());
        assert_eq!(runtime.world_snapshot().gold, 6_000);

        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .unwrap();
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .unwrap();
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .unwrap();
        let saved_entity = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        assert_eq!((saved_entity.x, saved_entity.y), (40, 41));
        assert_eq!(runtime.world_snapshot().gold, 6_000);
    }

    #[test]
    fn shared_runtime_npc_teleport_checkpoint_failure_rolls_back_before_dispatch() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let map = ZoneMapMetadata {
            map_index: 1,
            file_name: "0".to_string(),
            title: "BichonProvince".to_string(),
            mini_map: 1,
            big_map: 101,
            lights: 2,
            map_dark_light: 0,
            music: 0,
            weather: 0,
        };
        assert!(zone_state.lock().unwrap().zone_manager.install_empty_zone(
            ZoneRuntime::new_with_collision_and_npc_teleport_config(
                ZoneKey::for_map("0"),
                ZoneCollision::unbounded(),
                ZoneNpcTeleportConfig {
                    enabled: true,
                    cost: 3_000,
                    maps: BTreeMap::from([("0".to_string(), map)]),
                    destinations: vec![ZoneNpcTeleportDestination {
                        map_file_name: "0".to_string(),
                        object_id: 900,
                    }],
                },
            )
        ));

        let mut runtime = shared_session_runtime(Arc::clone(&zone_state));
        start_demo_runtime(&mut runtime);
        let mut checkpoint = runtime.inner.active_character_checkpoint().unwrap();
        checkpoint.gold = 9_000;
        runtime
            .inner
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let session_id = runtime.current_zone_session_id().unwrap();
        let old_zone_transform = zone_state
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&session_id)
            .unwrap();
        zone_state
            .lock()
            .unwrap()
            .zone_manager
            .handle(ZoneCommand::SyncSharedObjects {
                session_id: session_id.clone(),
                packets: vec![ServerPacket::ObjectNpc {
                    info: NpcInfo {
                        object_id: 900,
                        name: "Teleport Guide".to_string(),
                        name_colour_argb: -1,
                        image: 12,
                        colour_argb: -1,
                        location: Point { x: 40, y: 40 },
                        direction: MirDirection::Down,
                        quest_ids: Vec::new(),
                    },
                }],
                include_owner: false,
                now_ms: 1,
            });

        runtime.fail_next_npc_teleport_checkpoint_restore = true;
        let rejected = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TeleportToNpc {
                object_id: 900,
            }))
            .unwrap();

        assert!(rejected.is_empty());
        assert_eq!(runtime.world_snapshot().gold, 9_000);
        assert_eq!(
            zone_state
                .lock()
                .unwrap()
                .zone_manager
                .player_transform(&session_id),
            Some(old_zone_transform.clone())
        );
        let private_player = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        assert_eq!(
            (private_player.x, private_player.y, private_player.direction),
            (
                old_zone_transform.0.x,
                old_zone_transform.0.y,
                old_zone_transform.1
            )
        );

        let retry = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TeleportToNpc {
                object_id: 900,
            }))
            .unwrap();
        assert!(matches!(
            retry.first(),
            Some(ServerPacket::LoseGold { gold: 3_000 })
        ));
        assert_eq!(runtime.world_snapshot().gold, 6_000);
        assert_eq!(
            zone_state
                .lock()
                .unwrap()
                .zone_manager
                .player_transform(&session_id),
            Some((Point { x: 40, y: 41 }, MirDirection::Down))
        );
    }

    #[test]
    fn shared_runtime_real_disabled_npc_teleport_is_silent_and_zero_mutation() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        start_demo_runtime(&mut runtime);
        let mut checkpoint = runtime.inner.active_character_checkpoint().unwrap();
        checkpoint.gold = 5_000;
        runtime
            .inner
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let before = runtime.world_snapshot();

        let packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::TeleportToNpc {
                object_id: 1,
            }))
            .unwrap();
        let after = runtime.world_snapshot();

        assert!(packets.is_empty());
        assert_eq!(after.gold, before.gold);
        let before_player = before
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        let after_player = after
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        assert_eq!(
            (after_player.x, after_player.y, after_player.direction),
            (before_player.x, before_player.y, before_player.direction)
        );
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
        let movement_sender = super::spawn_shared_zone_owner_with_cadence(
            &ZoneId::new("test-shared-zone"),
            zone_state.clone(),
            Duration::from_secs(60 * 60),
        );
        SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(GatewayConfig::default()),
            zone_state: zone_state.clone(),
            account_inventory_service,
            npc_world_service,
            economy_execution_context: None,
            last_ground_drop_projection_reconciliation_identity: None,
            trade_projection_reconciliation_state: TradeProjectionReconciliationState::Unknown,
            movement_ingress: super::SharedZoneMovementIngress::new(movement_sender, zone_state),
            shared_skill_item_request_seq: 0,
            force_next_zone_transform_sync: false,
            last_shared_entity_ids_by_map: Default::default(),
            last_shared_drop_ids_by_map: Default::default(),
            local_ground_drop_zone_ids: Default::default(),
            retired_local_ground_drop_ids: Default::default(),
            owner_dead_entity_ids: Default::default(),
            last_game_shop_purchase_outcome: None,
            fail_next_npc_teleport_checkpoint_restore: false,
        }
    }

    #[test]
    fn shared_in_process_runtime_delegates_typed_game_shop_outcome() {
        let zone_state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(zone_state);
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("login should execute");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("StartGame should execute");

        let execution = runtime
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }))
            .expect("shared game-shop command should execute once");
        let outcome = execution
            .game_shop_purchase_outcome
            .expect("shared runtime must retain the typed purchase outcome");
        assert_eq!(
            (outcome.g_index, outcome.quantity, outcome.price_type),
            (31, 1, 1)
        );
    }

    fn start_demo_character(session: &mut GatewaySession) {
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    }

    fn start_new_runtime_handle(runtime: &mut ZoneRuntimeHandle, account_id: &str, name: &str) {
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
            .expect("new account should execute through runtime handle");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: account_id.to_string(),
                password: account_id.to_string(),
            }))
            .expect("login should execute through runtime handle");
        let character_index = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
                name: name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            }))
            .expect("new character should execute through runtime handle")
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("new character should return an index");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index,
            }))
            .expect("start game should execute through runtime handle");
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
        session.handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
        });
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

    fn equip_runtime_crystal_items(
        runtime: &mut SharedInProcessZoneSessionRuntime,
        items: &[(&str, mir2_simulation::EquipmentSlot, u32)],
    ) {
        equip_runtime_crystal_items_for_class(runtime, MirClass::Taoist, items);
    }

    fn equip_runtime_crystal_items_for_class(
        runtime: &mut SharedInProcessZoneSessionRuntime,
        class: MirClass,
        items: &[(&str, mir2_simulation::EquipmentSlot, u32)],
    ) {
        let snapshot = runtime.inner.world_snapshot();
        let identity = runtime
            .inner
            .active_identity()
            .expect("equipment fixture requires an active character");
        let player = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("equipment fixture requires an in-world player");
        let equipment_items_json = items
            .iter()
            .map(|(name, slot, quantity)| {
                let template = mir2_game_data::crystal_item_by_name(name)
                    .expect("equipment fixture template should exist");
                serde_json::json!({
                    "key": format!("crystal-item-{}", template.item_index),
                    "slot": slot,
                    "quantity": quantity,
                    "name": template.name,
                    "icon": template.image,
                    "shape": u16::try_from(template.shape).ok(),
                    "description": template.tooltip.unwrap_or_default(),
                    "durability_current": template.durability.max(1),
                    "durability_max": template.durability.max(1),
                    "attack": 0,
                    "defence": 0
                })
                .to_string()
            })
            .collect::<Vec<_>>();
        let state = serde_json::json!({
            "character": {
                "name": identity.character_name,
                "level": 35,
                "class": class,
                "gender": "Male"
            },
            "mapFileName": snapshot.map_file_name.unwrap_or_else(|| "0".to_string()),
            "mapTitle": snapshot.map_title.unwrap_or_else(|| "BichonProvince".to_string()),
            "position": { "x": player.x, "y": player.y },
            "direction": player.direction,
            "hp": snapshot.player_hp.unwrap_or(100),
            "maxHp": snapshot.player_max_hp.unwrap_or(100),
            "mp": snapshot.player_mp.unwrap_or(100),
            "maxMp": snapshot.player_max_mp.unwrap_or(100),
            "experience": snapshot.player_experience,
            "maxExperience": snapshot.player_max_experience,
            "gold": snapshot.gold,
            "credit": snapshot.credit,
            "inventoryItemsJson": [],
            "beltItemsJson": [],
            "storageItemsJson": [],
            "equipmentItemsJson": equipment_items_json
        });
        runtime
            .execute(WorldCommand::Stage5Command {
                action: "qa.applyNativeState".to_string(),
                args: vec![state.to_string()],
            })
            .expect("equipment fixture should apply through the test runtime");
    }

    fn prepare_gateway_range_fixture(
        runtime: &mut SharedInProcessZoneSessionRuntime,
        distance: i32,
    ) -> (WorldEntitySnapshot, Point) {
        let target = runtime
            .inner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster && entity.hp.is_some_and(|hp| hp > 1)
            })
            .expect("starter scene should expose a live range target");
        let attacker_position = Point {
            x: target.x.saturating_sub(distance),
            y: target.y,
        };
        let session_id = runtime
            .current_zone_session_id()
            .expect("range fixture should have a Zone session");
        let _ = runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position: attacker_position.clone(),
                direction: MirDirection::Right,
            },
            false,
        );
        runtime
            .inner
            .force_authoritative_player_transform(attacker_position.clone(), MirDirection::Right);
        (target, attacker_position)
    }

    fn gateway_range_attack_command(
        target: &WorldEntitySnapshot,
        attacker_position: Point,
    ) -> WorldCommand {
        WorldCommand::ClientPacket(ClientPacket::RangeAttack {
            direction: MirDirection::Right,
            location: attacker_position,
            target_id: target.object_id,
            target_location: Point {
                x: target.x,
                y: target.y,
            },
        })
    }

    fn shared_monster_entity(object_id: u32) -> WorldEntitySnapshot {
        WorldEntitySnapshot {
            object_id,
            kind: WorldEntityKind::Monster,
            name: "Deer".to_string(),
            owner_name: None,
            ai: Some(1),
            x: 329,
            y: 269,
            direction: MirDirection::Down,
            class: None,
            gender: None,
            level: None,
            riding_mount: None,
            can_mount_attack: None,
            has_class_weapon: None,
            dazed: None,
            fishing: None,
            hp: Some(12),
            max_hp: Some(12),
            light: 0,
            name_colour_argb: -1,
            dead: false,
            disposition: WorldEntityDisposition::Neutral,
            sprite: None,
            quest_ids: Vec::new(),
            quest_icon: None,
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
            transform_type: -1,
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
            ai: None,
            x,
            y,
            direction: MirDirection::Down,
            class: Some(MirClass::Warrior),
            gender: Some(MirGender::Male),
            level: Some(7),
            riding_mount: None,
            can_mount_attack: None,
            has_class_weapon: None,
            dazed: None,
            fishing: None,
            hp: None,
            max_hp: None,
            light: 3,
            name_colour_argb: -1,
            dead: false,
            disposition: WorldEntityDisposition::Friendly,
            sprite: None,
            quest_ids: Vec::new(),
            quest_icon: None,
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

    mod abnormal_teardown_zone_drain_tests {
        include!("abnormal_teardown_zone_drain_tests.rs");
    }
}
