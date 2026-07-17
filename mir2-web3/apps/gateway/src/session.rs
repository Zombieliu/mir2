use std::any::Any;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, Point, ServerPacket};
use mir2_simulation::{
    ActiveSessionIdentity, SimulationConfig, WorldCommand, WorldCommandExecution, WorldSnapshot,
    ZoneRuntimeHandle,
};

use crate::events::{GatewayGameplayEventPublisher, SharedGameplayEventSink};
use crate::routing::{
    shared_zone_movement_ingress, sync_zone_movement_transform, InProcessZoneOwnerCommandClient,
    SharedZoneLiveOutboundRegistration, SharedZoneLiveOutboundSender, SharedZoneMovementIngress,
    SharedZoneOwnerCommandClient, SharedZoneOwnerLeaseAuthority, ZoneId, ZoneOwnerCommandRequest,
    ZoneOwnerLease, ZoneRegistry,
};

pub type GatewayConfig = SimulationConfig;

pub(crate) fn catch_gateway_panic<T>(
    operation: &'static str,
    work: impl FnOnce() -> T,
) -> Result<T, String> {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(value) => Ok(value),
        Err(payload) => {
            let message = panic_payload_message(payload.as_ref());
            let error = format!("gateway session panic during {operation}: {message}");
            eprintln!("{error}");
            Err(error)
        }
    }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[derive(Clone)]
pub(crate) struct GatewayZoneMovementIngress {
    ingress: SharedZoneMovementIngress,
    zone_owner_lease: ZoneOwnerLease,
    zone_owner_lease_authority: Option<SharedZoneOwnerLeaseAuthority>,
    gameplay_event_publisher: Option<GatewayGameplayEventPublisher>,
}

impl GatewayZoneMovementIngress {
    pub(crate) fn try_execute(
        &self,
        packet: ClientPacket,
    ) -> Result<Option<WorldCommandExecution>, String> {
        if let Some(authority) = &self.zone_owner_lease_authority {
            authority.validate_owner_lease(&self.zone_owner_lease)?;
        }
        let execution = self.ingress.try_execute(packet)?;
        if let (Some(publisher), Some(execution)) =
            (&self.gameplay_event_publisher, execution.as_ref())
        {
            publisher.publish(&execution.outcome);
        }
        Ok(execution)
    }

    pub(crate) fn register_live_outbound(
        &self,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<SharedZoneLiveOutboundRegistration>, String> {
        if let Some(authority) = &self.zone_owner_lease_authority {
            authority.validate_owner_lease(&self.zone_owner_lease)?;
        }
        self.ingress.register_live_outbound(sender)
    }
}

pub struct GatewaySession {
    session_id: String,
    zone_id: ZoneId,
    zone_owner_lease: ZoneOwnerLease,
    zone_owner_lease_authority: Option<SharedZoneOwnerLeaseAuthority>,
    zone_owner_command_client: SharedZoneOwnerCommandClient,
    zone_owner_heartbeat_interval_ms: Option<u64>,
    next_zone_owner_heartbeat_at_ms: u64,
    runtime: ZoneRuntimeHandle,
    gameplay_event_publisher: Option<GatewayGameplayEventPublisher>,
}

impl fmt::Debug for GatewaySession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewaySession")
            .field("session_id", &self.session_id)
            .field("zone_id", &self.zone_id)
            .field("zone_owner_lease", &self.zone_owner_lease)
            .field("zone_owner_lease_authority", &"ZoneOwnerLeaseAuthority")
            .field("zone_owner_command_client", &"ZoneOwnerCommandClient")
            .field("runtime", &"WorldRuntime")
            .finish()
    }
}

impl GatewaySession {
    pub fn new(config: GatewayConfig) -> Self {
        Self::new_with_zone_registry(config, &ZoneRegistry::in_process())
    }

    pub fn new_with_zone_registry(config: GatewayConfig, registry: &ZoneRegistry) -> Self {
        let routed = registry.open_session(config);
        Self::with_routed_world_runtime_and_owner_authority(
            routed.zone_id,
            routed.owner_lease,
            Some(routed.owner_lease_authority),
            routed.runtime,
        )
    }

    pub fn with_routed_world_runtime(zone_id: ZoneId, runtime: ZoneRuntimeHandle) -> Self {
        let zone_owner_lease = ZoneOwnerLease::in_process(&zone_id);
        Self::with_routed_world_runtime_and_owner(zone_id, zone_owner_lease, runtime)
    }

    pub fn with_routed_world_runtime_and_owner(
        zone_id: ZoneId,
        zone_owner_lease: ZoneOwnerLease,
        runtime: ZoneRuntimeHandle,
    ) -> Self {
        Self::with_routed_world_runtime_and_owner_authority(
            zone_id,
            zone_owner_lease,
            None,
            runtime,
        )
    }

    pub fn with_routed_world_runtime_and_owner_authority(
        zone_id: ZoneId,
        zone_owner_lease: ZoneOwnerLease,
        zone_owner_lease_authority: Option<SharedZoneOwnerLeaseAuthority>,
        runtime: ZoneRuntimeHandle,
    ) -> Self {
        let zone_owner_command_client =
            default_zone_owner_command_client(zone_owner_lease_authority.as_ref());
        Self {
            session_id: next_gateway_session_id(),
            zone_id,
            zone_owner_lease,
            zone_owner_lease_authority,
            zone_owner_command_client,
            zone_owner_heartbeat_interval_ms: None,
            next_zone_owner_heartbeat_at_ms: 0,
            runtime,
            gameplay_event_publisher: None,
        }
    }

    pub fn with_routed_world_runtime_and_event_sink(
        zone_id: ZoneId,
        runtime: ZoneRuntimeHandle,
        gameplay_event_sink: SharedGameplayEventSink,
    ) -> Self {
        let zone_owner_lease = ZoneOwnerLease::in_process(&zone_id);
        Self::with_routed_world_runtime_and_owner_authority_and_event_sink(
            zone_id,
            zone_owner_lease,
            None,
            runtime,
            gameplay_event_sink,
        )
    }

    pub fn with_routed_world_runtime_and_owner_and_event_sink(
        zone_id: ZoneId,
        zone_owner_lease: ZoneOwnerLease,
        runtime: ZoneRuntimeHandle,
        gameplay_event_sink: SharedGameplayEventSink,
    ) -> Self {
        Self::with_routed_world_runtime_and_owner_authority_and_event_sink(
            zone_id,
            zone_owner_lease,
            None,
            runtime,
            gameplay_event_sink,
        )
    }

    pub fn with_routed_world_runtime_and_owner_authority_and_event_sink(
        zone_id: ZoneId,
        zone_owner_lease: ZoneOwnerLease,
        zone_owner_lease_authority: Option<SharedZoneOwnerLeaseAuthority>,
        runtime: ZoneRuntimeHandle,
        gameplay_event_sink: SharedGameplayEventSink,
    ) -> Self {
        let zone_owner_command_client =
            default_zone_owner_command_client(zone_owner_lease_authority.as_ref());
        let gameplay_event_publisher =
            GatewayGameplayEventPublisher::new(zone_id.clone(), gameplay_event_sink);
        Self {
            session_id: next_gateway_session_id(),
            zone_id,
            zone_owner_lease,
            zone_owner_lease_authority,
            zone_owner_command_client,
            zone_owner_heartbeat_interval_ms: None,
            next_zone_owner_heartbeat_at_ms: 0,
            runtime,
            gameplay_event_publisher: Some(gameplay_event_publisher),
        }
    }

    pub fn new_with_zone_registry_and_event_sink(
        config: GatewayConfig,
        registry: &ZoneRegistry,
        gameplay_event_sink: SharedGameplayEventSink,
    ) -> Self {
        let routed = registry.open_session(config);
        Self::with_routed_world_runtime_and_owner_authority_and_event_sink(
            routed.zone_id,
            routed.owner_lease,
            Some(routed.owner_lease_authority),
            routed.runtime,
            gameplay_event_sink,
        )
    }

    pub fn zone_id(&self) -> &ZoneId {
        &self.zone_id
    }

    pub fn zone_owner_lease(&self) -> &ZoneOwnerLease {
        &self.zone_owner_lease
    }

    pub fn renew_zone_owner_lease(&mut self) -> Result<&ZoneOwnerLease, String> {
        self.renew_zone_owner_lease_at(gateway_now_ms())
    }

    pub fn renew_zone_owner_lease_at(&mut self, now_ms: u64) -> Result<&ZoneOwnerLease, String> {
        if let Some(authority) = &self.zone_owner_lease_authority {
            self.zone_owner_lease =
                authority.renew_owner_lease_at(&self.zone_owner_lease, now_ms)?;
        }
        Ok(&self.zone_owner_lease)
    }

    pub fn configure_zone_owner_heartbeat(&mut self, interval_ms: u64, now_ms: u64) {
        let interval_ms = interval_ms.max(1);
        self.zone_owner_heartbeat_interval_ms = Some(interval_ms);
        self.next_zone_owner_heartbeat_at_ms = now_ms.saturating_add(interval_ms);
    }

    pub fn renew_zone_owner_lease_if_due(&mut self) -> Result<bool, String> {
        self.renew_zone_owner_lease_if_due_at(gateway_now_ms())
    }

    pub fn renew_zone_owner_lease_if_due_at(&mut self, now_ms: u64) -> Result<bool, String> {
        let Some(interval_ms) = self.zone_owner_heartbeat_interval_ms else {
            return Ok(false);
        };
        if now_ms < self.next_zone_owner_heartbeat_at_ms {
            return Ok(false);
        }

        self.renew_zone_owner_lease_at(now_ms)?;
        self.next_zone_owner_heartbeat_at_ms = now_ms.saturating_add(interval_ms);
        Ok(true)
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn on_connect(&self) -> Vec<ServerPacket> {
        self.runtime.on_connect()
    }

    pub(crate) fn zone_movement_ingress(&self) -> Option<GatewayZoneMovementIngress> {
        shared_zone_movement_ingress(&self.runtime).map(|ingress| GatewayZoneMovementIngress {
            ingress,
            zone_owner_lease: self.zone_owner_lease.clone(),
            zone_owner_lease_authority: self.zone_owner_lease_authority.clone(),
            gameplay_event_publisher: self.gameplay_event_publisher.clone(),
        })
    }

    pub fn handle_packet(&mut self, packet: ClientPacket) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::ClientPacket(packet))
    }

    pub fn passkey_login(&mut self, account_id: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::PasskeyLogin {
            account_id: account_id.to_string(),
        })
    }

    pub fn execute_with_outcome(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let owner_lease = self.zone_owner_lease.clone();
        self.execute_zone_owner_command(ZoneOwnerCommandRequest::direct(owner_lease, command))
    }

    pub fn execute_with_zone_owner_lease(
        &mut self,
        zone_owner_lease: &ZoneOwnerLease,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        self.execute_zone_owner_command(ZoneOwnerCommandRequest::direct(
            zone_owner_lease.clone(),
            command,
        ))
    }

    pub fn execute_production_player_command(
        &mut self,
        authenticated: bool,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let owner_lease = self.zone_owner_lease.clone();
        self.execute_zone_owner_command(ZoneOwnerCommandRequest::production_player(
            owner_lease,
            authenticated,
            command,
        ))
    }

    pub fn execute_production_player_command_with_zone_owner_lease(
        &mut self,
        zone_owner_lease: &ZoneOwnerLease,
        authenticated: bool,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        self.execute_zone_owner_command(ZoneOwnerCommandRequest::production_player(
            zone_owner_lease.clone(),
            authenticated,
            command,
        ))
    }

    pub fn validate_zone_owner_lease(
        &self,
        zone_owner_lease: &ZoneOwnerLease,
    ) -> Result<(), String> {
        if zone_owner_lease == &self.zone_owner_lease {
            if let Some(authority) = &self.zone_owner_lease_authority {
                authority.validate_owner_lease(zone_owner_lease)?;
            }
            return Ok(());
        }

        Err(format!(
            "stale zone owner lease for zone {}: expected owner {} fencing token {}, got zone {} owner {} fencing token {}; command rejected before ZoneRuntime",
            self.zone_id,
            self.zone_owner_lease.owner_id(),
            self.zone_owner_lease.fencing_token(),
            zone_owner_lease.zone_id(),
            zone_owner_lease.owner_id(),
            zone_owner_lease.fencing_token()
        ))
    }

    pub fn move_to(&mut self, x: i32, y: i32, running: bool) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::MoveTo {
            position: Point { x, y },
            running,
        })
    }

    pub fn attack(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Attack { object_id })
    }

    pub fn interact(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Interact { object_id })
    }

    pub fn select_npc_dialog_target(&mut self, target: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::SelectNpcDialog {
            target: target.to_string(),
        })
    }

    pub fn submit_npc_input(&mut self, value: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::SubmitNpcInput {
            value: value.to_string(),
        })
    }

    pub fn pick_up(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::PickUp { object_id })
    }

    pub fn use_item(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::UseItem {
            key: key.to_string(),
        })
    }

    pub fn drop_item(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::DropItem {
            key: key.to_string(),
        })
    }

    pub fn delete_character(&mut self, character_index: i32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::DeleteCharacter { character_index })
    }

    pub fn cast_skill(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::CastSkill {
            key: key.to_string(),
        })
    }

    pub fn transfer_map(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::TransferMap {
            key: key.to_string(),
        })
    }

    pub fn stage5_command(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Stage5Command {
            action: action.to_string(),
            args,
        })
    }

    pub fn tick(&mut self) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Tick)
    }

    pub fn set_language(&mut self, language: &str) -> Result<(), String> {
        self.execute_world_command(WorldCommand::SetLanguage {
            language: language.to_string(),
        })
        .map(|_| ())
    }

    pub fn world_snapshot(&self) -> WorldSnapshot {
        self.zone_owner_command_client
            .world_snapshot(&self.runtime)
            .expect("zone owner world snapshot should not fail")
    }

    pub fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.zone_owner_command_client
            .active_identity(&self.runtime)
            .expect("zone owner active identity should not fail")
    }

    pub fn save_active_character(&mut self) {
        sync_zone_movement_transform(&mut self.runtime)
            .expect("zone movement transform sync should not fail before save");
        self.zone_owner_command_client
            .save_active_character(&self.runtime)
            .expect("zone owner save should not fail");
    }

    pub fn refresh_active_external_mail(&mut self) -> bool {
        self.zone_owner_command_client
            .refresh_active_external_mail(&mut self.runtime)
            .expect("zone owner mail refresh should not fail")
    }

    fn execute_infallible(&mut self, command: WorldCommand) -> Vec<ServerPacket> {
        self.execute_world_command(command)
            .map(|execution| execution.packets)
            .expect("non-language world runtime command should not fail")
    }

    fn execute_world_command(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let owner_lease = self.zone_owner_lease.clone();
        self.execute_zone_owner_command(ZoneOwnerCommandRequest::direct(owner_lease, command))
    }

    fn execute_zone_owner_command(
        &mut self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.validate_zone_owner_lease(request.owner_lease())?;
        let execution = self
            .zone_owner_command_client
            .execute(&mut self.runtime, request)?;
        self.publish_gameplay_event(&execution);
        Ok(execution)
    }

    fn publish_gameplay_event(&self, execution: &WorldCommandExecution) {
        if let Some(publisher) = &self.gameplay_event_publisher {
            publisher.publish(&execution.outcome);
        }
    }
}

fn default_zone_owner_command_client(
    zone_owner_lease_authority: Option<&SharedZoneOwnerLeaseAuthority>,
) -> SharedZoneOwnerCommandClient {
    let client = if let Some(authority) = zone_owner_lease_authority {
        InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority.clone())
    } else {
        InProcessZoneOwnerCommandClient::new()
    };
    std::sync::Arc::new(client)
}

static NEXT_GATEWAY_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn next_gateway_session_id() -> String {
    let sequence = NEXT_GATEWAY_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let now_ms = gateway_now_ms();
    format!("gateway-{}-{now_ms}-{sequence}", std::process::id())
}

fn gateway_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{GatewayConfig, GatewaySession};
    use crate::{
        CharacterRecord, HostedZoneOwnerCommandClient, InMemoryGameplayEventSink,
        InMemoryZoneOwnerLeaseAuthority, InProcessZoneOwnerCommandClient,
        InProcessZoneRuntimeFactory, RpcZoneOwnerCommandClient, SharedGameplayEventSink,
        SharedSessionRouter, SharedZoneOwnerCommandClient, SharedZoneOwnerLeaseAuthority,
        SharedZoneOwnerRpcTransport, SharedZoneRuntimeFactory, SingleZoneSessionRouter, ZoneId,
        ZoneOwnerCommandClient, ZoneOwnerCommandMode, ZoneOwnerCommandRequest, ZoneOwnerLease,
        ZoneOwnerLeaseAuthority, ZoneRegistry, ZoneRuntimeFactory,
    };
    use mir2_protocol::{ChatType, ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
    use mir2_simulation::{
        WorldCommand, WorldCommandExecution, WorldCommandKind, ZoneRuntimeHandle,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct RecordingZoneOwnerCommandClient {
        calls: Mutex<Vec<(ZoneOwnerLease, ZoneOwnerCommandMode)>>,
    }

    impl RecordingZoneOwnerCommandClient {
        fn calls(&self) -> Vec<(ZoneOwnerLease, ZoneOwnerCommandMode)> {
            self.calls
                .lock()
                .expect("recording command client mutex should not be poisoned")
                .clone()
        }
    }

    impl ZoneOwnerCommandClient for RecordingZoneOwnerCommandClient {
        fn execute(
            &self,
            runtime: &mut ZoneRuntimeHandle,
            request: ZoneOwnerCommandRequest,
        ) -> Result<WorldCommandExecution, String> {
            self.calls
                .lock()
                .expect("recording command client mutex should not be poisoned")
                .push((request.owner_lease().clone(), request.mode()));
            ZoneOwnerCommandClient::execute(
                &InProcessZoneOwnerCommandClient::new(),
                runtime,
                request,
            )
        }
    }

    #[test]
    fn gateway_panic_boundary_returns_operation_error() {
        let result = super::catch_gateway_panic("unit-test", || panic!("boom"));

        assert_eq!(
            result.expect_err("panic should be caught"),
            "gateway session panic during unit-test: boom"
        );
    }

    #[test]
    fn login_returns_character_list() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });

        match &packets[0] {
            ServerPacket::LoginSuccess { characters } => {
                assert_eq!(characters.len(), 1);
                assert_eq!(characters[0].name, "Scout");
            }
            other => panic!("unexpected packet: {other:?}"),
        }
    }

    #[test]
    fn gateway_session_exposes_world_command_outcome() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });

        session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("hosted owner runtime should execute login");
        let execution = session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("gateway should expose world command outcome");

        assert_eq!(
            execution.outcome.command_kind,
            WorldCommandKind::ClientPacket("StartGame")
        );
        assert_eq!(execution.outcome.packet_count, execution.packets.len());
        assert_eq!(
            execution
                .outcome
                .active_identity
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
    }

    #[test]
    fn gateway_session_accepts_current_zone_owner_fenced_command() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let owner_lease = session.zone_owner_lease().clone();

        let execution = session
            .execute_with_zone_owner_lease(
                &owner_lease,
                WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
            )
            .expect("current owner lease should execute");

        assert!(execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        assert_eq!(
            execution.outcome.command_kind,
            WorldCommandKind::ClientPacket("StartGame")
        );
    }

    #[test]
    fn gateway_session_dispatches_valid_zone_owner_request_through_command_client() {
        let client = Arc::new(RecordingZoneOwnerCommandClient::default());
        let mut session = GatewaySession::new(GatewayConfig::default());
        session.zone_owner_command_client = client.clone() as SharedZoneOwnerCommandClient;
        let owner_lease = session.zone_owner_lease().clone();

        session
            .execute_production_player_command(true, WorldCommand::Tick)
            .expect("valid owner request should execute through command client");

        assert_eq!(
            client.calls(),
            vec![(
                owner_lease,
                ZoneOwnerCommandMode::ProductionPlayer {
                    authenticated: true
                }
            )]
        );
    }

    #[test]
    fn hosted_zone_owner_command_client_executes_on_owner_runtime_not_gateway_runtime() {
        let zone_id = ZoneId::primary();
        let owner_lease = ZoneOwnerLease::in_process(&zone_id);
        let gateway_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let owner_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let hosted_client = Arc::new(HostedZoneOwnerCommandClient::new(owner_runtime));
        let mut session = GatewaySession::with_routed_world_runtime_and_owner(
            zone_id,
            owner_lease,
            gateway_runtime,
        );
        session.zone_owner_command_client = hosted_client.clone() as SharedZoneOwnerCommandClient;

        session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("hosted owner runtime should execute login");
        let execution = session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("hosted owner runtime should execute command");

        assert!(execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        assert_eq!(
            execution
                .outcome
                .active_identity
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
        assert_eq!(
            hosted_client
                .active_identity()
                .expect("hosted runtime identity should be readable")
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
        assert_eq!(
            hosted_client
                .world_snapshot()
                .expect("hosted runtime snapshot should be readable")
                .map_file_name
                .as_deref(),
            Some("0")
        );
        assert_eq!(
            session
                .active_identity()
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
        assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
        assert!(
            session.runtime.active_identity().is_none(),
            "Gateway's local runtime must not be the command mutation target"
        );
    }

    #[test]
    fn rpc_zone_owner_command_client_executes_on_transport_owner_runtime() {
        let zone_id = ZoneId::primary();
        let owner_lease = ZoneOwnerLease::in_process(&zone_id);
        let gateway_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let owner_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let hosted_transport = Arc::new(HostedZoneOwnerCommandClient::new(owner_runtime));
        let rpc_client = Arc::new(RpcZoneOwnerCommandClient::new(
            hosted_transport.clone() as SharedZoneOwnerRpcTransport
        ));
        let mut session = GatewaySession::with_routed_world_runtime_and_owner(
            zone_id,
            owner_lease,
            gateway_runtime,
        );
        session.zone_owner_command_client = rpc_client as SharedZoneOwnerCommandClient;

        session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("RPC transport owner runtime should execute login");
        let execution = session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("RPC transport owner runtime should execute command");

        assert!(execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        assert_eq!(
            hosted_transport
                .active_identity()
                .expect("transport owner runtime identity should be readable")
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
        assert_eq!(
            session
                .active_identity()
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
        assert!(
            session.runtime.active_identity().is_none(),
            "Gateway's local runtime must not be the RPC transport mutation target"
        );
    }

    #[test]
    fn hosted_zone_owner_command_client_rejects_stale_request_at_owner_boundary() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let zone_id = ZoneId::primary();
        let owner_lease = authority.owner_lease(&zone_id);
        let owner_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let hosted_client = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            owner_runtime,
            authority.clone() as SharedZoneOwnerLeaseAuthority,
        );
        let mut gateway_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);

        authority.handoff_zone_owner(&zone_id, "zone-owner:next");
        let error = ZoneOwnerCommandClient::execute(
            &hosted_client,
            &mut gateway_runtime,
            ZoneOwnerCommandRequest::direct(owner_lease, WorldCommand::Tick),
        )
        .expect_err("hosted owner boundary should reject stale fenced request");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner zone-owner:next fencing token 2"));
        assert!(hosted_client
            .active_identity()
            .expect("hosted runtime identity should be readable")
            .is_none());
    }

    #[test]
    fn rpc_zone_owner_command_client_rejects_stale_request_at_transport_boundary() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let zone_id = ZoneId::primary();
        let owner_lease = authority.owner_lease(&zone_id);
        let owner_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let hosted_transport = Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
            owner_runtime,
            authority.clone() as SharedZoneOwnerLeaseAuthority,
        ));
        let rpc_client =
            RpcZoneOwnerCommandClient::new(hosted_transport.clone() as SharedZoneOwnerRpcTransport);
        let mut gateway_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);

        authority.handoff_zone_owner(&zone_id, "zone-owner:next");
        let error = ZoneOwnerCommandClient::execute(
            &rpc_client,
            &mut gateway_runtime,
            ZoneOwnerCommandRequest::direct(owner_lease, WorldCommand::Tick),
        )
        .expect_err("RPC transport owner boundary should reject stale fenced request");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner zone-owner:next fencing token 2"));
        assert!(hosted_transport
            .active_identity()
            .expect("transport owner runtime identity should be readable")
            .is_none());
    }

    #[test]
    fn hosted_zone_owner_runtime_handoff_moves_state_to_next_owner_host() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let zone_id = ZoneId::primary();
        let first_lease = authority.owner_lease(&zone_id);
        let gateway_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let first_owner_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let first_owner = Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
            first_owner_runtime,
            authority.clone() as SharedZoneOwnerLeaseAuthority,
        ));
        let first_rpc = Arc::new(RpcZoneOwnerCommandClient::new(
            first_owner.clone() as SharedZoneOwnerRpcTransport
        ));
        let mut first_gateway = GatewaySession::with_routed_world_runtime_and_owner_authority(
            zone_id.clone(),
            first_lease.clone(),
            Some(authority.clone() as SharedZoneOwnerLeaseAuthority),
            gateway_runtime,
        );
        first_gateway.zone_owner_command_client = first_rpc as SharedZoneOwnerCommandClient;

        first_gateway
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("first owner should execute login");
        first_gateway
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("first owner should enter game");
        assert_eq!(
            first_owner
                .active_identity()
                .expect("first owner identity should be readable before handoff")
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );

        let handed_runtime = first_owner
            .take_runtime_for_handoff()
            .expect("first owner should export runtime for handoff");
        assert!(first_owner
            .active_identity()
            .expect_err("first owner should be unavailable after handoff")
            .contains("already handed off"));

        let next_lease = authority.handoff_zone_owner(&zone_id, "zone-owner:next");
        let next_owner = Arc::new(
            HostedZoneOwnerCommandClient::from_handoff_with_owner_lease_authority(
                handed_runtime,
                authority.clone() as SharedZoneOwnerLeaseAuthority,
            ),
        );
        let next_rpc = Arc::new(RpcZoneOwnerCommandClient::new(
            next_owner.clone() as SharedZoneOwnerRpcTransport
        ));
        let next_gateway_runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let mut next_gateway = GatewaySession::with_routed_world_runtime_and_owner_authority(
            zone_id.clone(),
            next_lease,
            Some(authority.clone() as SharedZoneOwnerLeaseAuthority),
            next_gateway_runtime,
        );
        next_gateway.zone_owner_command_client = next_rpc as SharedZoneOwnerCommandClient;

        assert_eq!(
            next_gateway
                .active_identity()
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
        assert_eq!(
            next_gateway.world_snapshot().map_file_name.as_deref(),
            Some("0")
        );
        next_gateway
            .execute_with_outcome(WorldCommand::Tick)
            .expect("next owner should continue executing handed-off runtime");

        let error = next_owner
            .execute_request(ZoneOwnerCommandRequest::direct(
                first_lease,
                WorldCommand::Tick,
            ))
            .expect_err("next owner should reject stale pre-handoff lease");
        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner zone-owner:next fencing token 2"));
    }

    #[test]
    fn gateway_session_rejects_stale_zone_owner_before_command_client() {
        let client = Arc::new(RecordingZoneOwnerCommandClient::default());
        let mut session = GatewaySession::new(GatewayConfig::default());
        session.zone_owner_command_client = client.clone() as SharedZoneOwnerCommandClient;
        let expected = session.zone_owner_lease().clone();
        let stale = ZoneOwnerLease::new(
            expected.zone_id().clone(),
            expected.owner_id().to_string(),
            expected.fencing_token() + 1,
        );

        session
            .execute_with_zone_owner_lease(&stale, WorldCommand::Tick)
            .expect_err("stale owner request should be rejected before command client");

        assert!(client.calls().is_empty());
    }

    #[test]
    fn in_process_zone_owner_command_client_rejects_stale_request_at_owner_boundary() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let owner_lease = authority.owner_lease(&ZoneId::primary());
        authority.handoff_zone_owner(&ZoneId::primary(), "zone-owner:next");
        let client = InProcessZoneOwnerCommandClient::with_owner_lease_authority(
            authority.clone() as SharedZoneOwnerLeaseAuthority
        );
        let mut runtime = InProcessZoneRuntimeFactory
            .create_runtime(GatewayConfig::default(), &ZoneId::primary());

        let error = ZoneOwnerCommandClient::execute(
            &client,
            &mut runtime,
            ZoneOwnerCommandRequest::direct(owner_lease, WorldCommand::Tick),
        )
        .expect_err("owner boundary should reject stale fenced request");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner zone-owner:next fencing token 2"));
    }

    #[test]
    fn gateway_session_rejects_stale_zone_owner_fencing_token() {
        let event_sink = Arc::new(InMemoryGameplayEventSink::default());
        let shared_event_sink: SharedGameplayEventSink = event_sink.clone();
        let mut session = GatewaySession::new_with_zone_registry_and_event_sink(
            GatewayConfig::default(),
            &crate::ZoneRegistry::in_process(),
            shared_event_sink,
        );
        let expected = session.zone_owner_lease().clone();
        let stale = ZoneOwnerLease::new(
            expected.zone_id().clone(),
            expected.owner_id().to_string(),
            expected.fencing_token() + 1,
        );

        let error = session
            .execute_with_zone_owner_lease(
                &stale,
                WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
            )
            .expect_err("stale owner lease should be rejected before runtime execution");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("expected owner in-process:primary fencing token 1"));
        assert!(error.contains("got zone primary owner in-process:primary fencing token 2"));
        assert!(event_sink.list().is_empty());
        assert!(session.active_identity().is_none());
    }

    #[test]
    fn gateway_session_rejects_wrong_zone_owner_before_production_command() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let expected = session.zone_owner_lease().clone();
        let stale_owner = ZoneOwnerLease::new(
            expected.zone_id().clone(),
            "in-process:old-owner",
            expected.fencing_token(),
        );

        let error = session
            .execute_production_player_command_with_zone_owner_lease(
                &stale_owner,
                true,
                WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
            )
            .expect_err("wrong owner should be rejected before production runtime execution");

        assert!(error.contains("stale zone owner lease"));
        assert!(error.contains("got zone primary owner in-process:old-owner fencing token 1"));
        assert!(session.active_identity().is_none());
    }

    #[test]
    fn gateway_session_rejects_superseded_zone_owner_after_handoff() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let registry = ZoneRegistry::with_router_and_owner_lease_authority(
            ZoneId::primary(),
            Arc::new(InProcessZoneRuntimeFactory) as SharedZoneRuntimeFactory,
            Arc::new(SingleZoneSessionRouter) as SharedSessionRouter,
            authority.clone() as SharedZoneOwnerLeaseAuthority,
        );
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        let old_owner_lease = session.zone_owner_lease().clone();

        let next_owner = authority.handoff_zone_owner(&ZoneId::primary(), "zone-owner:next");

        assert_eq!(
            next_owner.fencing_token(),
            old_owner_lease.fencing_token() + 1
        );
        let error = session
            .execute_with_zone_owner_lease(
                &old_owner_lease,
                WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
            )
            .expect_err("superseded owner lease should be rejected by shared authority");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner zone-owner:next fencing token 2"));
        assert!(error.contains("got owner in-process:primary fencing token 1"));
        assert!(session.active_identity().is_none());
    }

    #[test]
    fn gateway_session_renews_current_zone_owner_lease() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let registry = ZoneRegistry::with_router_and_owner_lease_authority(
            ZoneId::primary(),
            Arc::new(InProcessZoneRuntimeFactory) as SharedZoneRuntimeFactory,
            Arc::new(SingleZoneSessionRouter) as SharedSessionRouter,
            authority.clone() as SharedZoneOwnerLeaseAuthority,
        );
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        let owner_lease = session.zone_owner_lease().clone();

        let renewed = session
            .renew_zone_owner_lease()
            .expect("current owner lease should renew")
            .clone();

        assert_eq!(renewed, owner_lease);
    }

    #[test]
    fn gateway_session_rejects_zone_owner_renewal_after_handoff() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let registry = ZoneRegistry::with_router_and_owner_lease_authority(
            ZoneId::primary(),
            Arc::new(InProcessZoneRuntimeFactory) as SharedZoneRuntimeFactory,
            Arc::new(SingleZoneSessionRouter) as SharedSessionRouter,
            authority.clone() as SharedZoneOwnerLeaseAuthority,
        );
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);

        authority.handoff_zone_owner(&ZoneId::primary(), "zone-owner:next");

        let error = session
            .renew_zone_owner_lease()
            .expect_err("old owner lease renewal should fail after handoff");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner zone-owner:next fencing token 2"));
    }

    #[test]
    fn gateway_session_heartbeat_renews_zone_owner_before_ttl_expiry() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::with_lease_ttl_ms(100));
        let zone_id = ZoneId::primary();
        let owner_lease = authority.owner_lease_at(&zone_id, 1_000);
        let runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let mut session = GatewaySession::with_routed_world_runtime_and_owner_authority(
            zone_id.clone(),
            owner_lease.clone(),
            Some(authority.clone() as SharedZoneOwnerLeaseAuthority),
            runtime,
        );

        session.configure_zone_owner_heartbeat(40, 1_000);

        assert!(!session
            .renew_zone_owner_lease_if_due_at(1_039)
            .expect("heartbeat should not renew before interval"));
        assert!(session
            .renew_zone_owner_lease_if_due_at(1_040)
            .expect("heartbeat should renew before TTL expiry"));
        assert_eq!(session.zone_owner_lease(), &owner_lease);
        assert_eq!(authority.owner_lease_at(&zone_id, 1_139), owner_lease);
        assert_eq!(
            authority.owner_lease_at(&zone_id, 1_140).fencing_token(),
            owner_lease.fencing_token() + 1
        );
    }

    #[test]
    fn gateway_session_heartbeat_rejects_expired_zone_owner() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::with_lease_ttl_ms(100));
        let zone_id = ZoneId::primary();
        let owner_lease = authority.owner_lease_at(&zone_id, 1_000);
        let runtime =
            InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone_id);
        let mut session = GatewaySession::with_routed_world_runtime_and_owner_authority(
            zone_id,
            owner_lease,
            Some(authority as SharedZoneOwnerLeaseAuthority),
            runtime,
        );

        session.configure_zone_owner_heartbeat(40, 1_000);

        let error = session
            .renew_zone_owner_lease_if_due_at(1_100)
            .expect_err("missed heartbeat should fail after TTL expiry");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner in-process:primary fencing token 2"));
    }

    #[test]
    fn gateway_session_publishes_gameplay_event_from_world_outcome() {
        let event_sink = Arc::new(InMemoryGameplayEventSink::default());
        let shared_event_sink: SharedGameplayEventSink = event_sink.clone();
        let mut session = GatewaySession::new_with_zone_registry_and_event_sink(
            GatewayConfig::default(),
            &crate::ZoneRegistry::in_process(),
            shared_event_sink,
        );
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        event_sink.drain();

        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let events = event_sink.list();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "gameplay.command.executed");
        assert_eq!(
            events[0].event_id,
            format!("primary:{}:2", events[0].snapshot_tick)
        );
        assert_eq!(events[0].schema_version, 1);
        assert_eq!(events[0].zone_id, "primary");
        assert_eq!(events[0].command_kind, "client.StartGame");
        assert_eq!(events[0].account_id.as_deref(), Some("demo"));
        assert_eq!(events[0].character_index, Some(0));
        assert_eq!(events[0].character_name.as_deref(), Some("Scout"));
        assert_eq!(events[0].packet_count, packets.len());
        assert_eq!(events[0].snapshot_tick, session.world_snapshot().tick);
    }

    #[test]
    fn gateway_session_movement_event_skips_snapshot_tick() {
        let event_sink = Arc::new(InMemoryGameplayEventSink::default());
        let shared_event_sink: SharedGameplayEventSink = event_sink.clone();
        let mut session = GatewaySession::new_with_zone_registry_and_event_sink(
            GatewayConfig::default(),
            &crate::ZoneRegistry::in_process(),
            shared_event_sink,
        );
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        event_sink.drain();

        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        let events = event_sink.list();

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command_kind, "client.Walk");
        assert_eq!(events[0].packet_count, packets.len());
        assert_eq!(events[0].snapshot_tick, 0);
        assert_eq!(events[0].event_id, "primary:0:3");
    }

    #[test]
    fn gateway_zone_movement_ingress_publishes_gameplay_event_once() {
        let event_sink = Arc::new(InMemoryGameplayEventSink::default());
        let shared_event_sink: SharedGameplayEventSink = event_sink.clone();
        let mut session = GatewaySession::new_with_zone_registry_and_event_sink(
            GatewayConfig::default(),
            &crate::ZoneRegistry::in_process(),
            shared_event_sink,
        );
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        event_sink.drain();

        let execution = session
            .zone_movement_ingress()
            .expect("shared runtime should expose movement ingress")
            .try_execute(ClientPacket::Walk {
                direction: MirDirection::Right,
            })
            .expect("movement ingress should execute")
            .expect("safe movement should not fall back");
        let events = event_sink.list();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "primary:0:3");
        assert_eq!(events[0].command_kind, "client.Walk");
        assert_eq!(events[0].account_id.as_deref(), Some("demo"));
        assert_eq!(events[0].character_index, Some(0));
        assert_eq!(events[0].character_name.as_deref(), Some("Scout"));
        assert_eq!(events[0].packet_count, execution.packets.len());
        assert_eq!(events[0].snapshot_tick, 0);
    }

    #[test]
    fn gateway_zone_movement_ingress_rejects_stale_owner_lease() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let shared_authority: SharedZoneOwnerLeaseAuthority = authority.clone();
        let registry = ZoneRegistry::in_process_with_owner_lease_authority(shared_authority);
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let ingress = session
            .zone_movement_ingress()
            .expect("shared runtime should expose movement ingress");

        authority.handoff_zone_owner_at(session.zone_id(), "replacement-owner", 1);
        let error = ingress
            .try_execute(ClientPacket::Walk {
                direction: MirDirection::Right,
            })
            .expect_err("stale ingress owner lease should be fenced");

        assert!(error.contains("stale zone owner lease for zone primary"));
        assert!(error.contains("current owner replacement-owner fencing token 2"));
    }

    #[test]
    fn new_character_is_added_and_returned() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::NewCharacter {
            name: "Blade".to_string(),
            gender: MirGender::Female,
            class: MirClass::Wizard,
        });

        match &packets[0] {
            ServerPacket::NewCharacterSuccess { char_info } => {
                assert_eq!(char_info.name, "Blade");
                assert_eq!(char_info.class, MirClass::Wizard);
            }
            other => panic!("unexpected packet: {other:?}"),
        }
    }

    #[test]
    fn start_game_emits_bootstrap_sequence() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        assert!(matches!(
            packets[0],
            ServerPacket::StartGame { result: 4, .. }
        ));
        assert!(matches!(
            &packets[1],
            ServerPacket::Chat {
                chat_type: ChatType::Hint,
                message,
                ..
            } if message == "Welcome to the Legend of Mir 2 Server."
        ));
        let map_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::MapInformation { .. }))
            .expect("bootstrap should include map information");
        let user_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::UserInformation { .. }))
            .expect("bootstrap should include user information");
        let base_stats_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::BaseStatsInfo { .. }))
            .expect("bootstrap should include base stats");
        assert!(map_index < user_index);
        assert!(user_index < base_stats_index);
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectPlayer { .. })));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectMonster { .. })));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectNpc { .. })));
    }

    #[test]
    fn walk_updates_self_location() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });

        match &packets[0] {
            ServerPacket::UserLocation { location } => {
                assert_eq!(location.position.x, 331);
                assert_eq!(location.position.y, 270);
                assert_eq!(location.direction, MirDirection::Right);
            }
            other => panic!("unexpected packet: {other:?}"),
        }
    }

    #[test]
    fn chat_before_start_game_rejects_without_packets() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::Chat {
            message: "hi".to_string(),
            linked_items: Vec::new(),
        });

        assert!(packets.is_empty());
    }

    #[test]
    fn chat_normal_message_emits_crystal_object_chat_only() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let packets = session.handle_packet(ClientPacket::Chat {
            message: "hi".to_string(),
            linked_items: Vec::new(),
        });

        assert_eq!(packets.len(), 1);
        assert!(matches!(
            &packets[0],
            ServerPacket::ObjectChat {
                text,
                chat_type: ChatType::Normal,
                ..
            } if text == "Scout: hi"
        ));
    }

    #[test]
    fn delete_character_removes_entry_from_character_list() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let _ = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        let _ = session.handle_packet(ClientPacket::NewCharacter {
            name: "Blade".to_string(),
            gender: MirGender::Female,
            class: MirClass::Wizard,
        });

        let packets = session.delete_character(1);

        assert!(matches!(
            packets[0],
            ServerPacket::DeleteCharacterSuccess { character_index: 1 }
        ));
    }

    #[test]
    fn drop_item_creates_ground_drop_packet() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let packets = session.drop_item("red-potion");

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectItem { .. })));
    }

    #[test]
    fn reexported_character_record_matches_expected_shape() {
        let record = CharacterRecord {
            index: 1,
            name: "Scout".to_string(),
            level: 7,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        };

        assert_eq!(record.name, "Scout");
    }
}
