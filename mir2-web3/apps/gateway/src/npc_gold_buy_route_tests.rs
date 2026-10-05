//! Pure-memory owner-route tests. Demo bootstrap is a fixture, not production authentication.
use super::*;
use crate::events::{GameplayEventSink, GameplayEventSinkStatus, GatewayGameplayEvent, InMemoryGameplayEventSink};
use crate::routing::{
    npc_gold_buy_test_access as shared_test, HostedZoneOwnerCommandClient,
    InMemoryZoneOwnerLeaseAuthority, InProcessZoneRuntimeFactory,
    SharedInProcessZoneRuntimeFactory, ZoneOwnerCommandClient, ZoneOwnerLeaseAuthority,
    ZoneRuntimeFactory,
};
use mir2_protocol::{MirDirection, UserItem};
use mir2_simulation::{
    InProcessWorldRuntime, NpcGoldBuyProcessingExecution, NpcGoldBuyProcessingOutcome,
    NpcGoldBuyRejection, VisibleNpcRecord, WorldCommandKind, WorldRuntime,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const NPC_ID: u32 = 4990;

fn config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.account_store_path = None;
    config.account_store_database_url = None;
    config.save_recovery_dir = None;
    config.visible_monsters.clear();
    config.map_hazards.clear();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: NPC_ID,
        name: "Wicked Trader".into(),
        image: 5,
        colour_argb: -1,
        position: mir2_protocol::Point { x: config.spawn.x + 1, y: config.spawn.y },
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
        save.position = config.spawn.clone();
        save.gold = 100_000;
        save.inventory_capacity = 86;
        save.inventory_items_json.clear();
        save.belt_items_json.clear();
        save.equipment_items_json.clear();
        save.equipment_items_explicit_empty = true;
    }
    config
}

fn login(session: &mut GatewaySession) {
    let packets = session.execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    })).unwrap().packets;
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

fn open_catalog(session: &mut GatewaySession) -> NpcGoldBuyRequest {
    session.execute_with_outcome(WorldCommand::Interact { object_id: NPC_ID }).unwrap();
    let packets = session.execute_with_outcome(WorldCommand::SelectNpcDialog {
        target: "@BuySell".into(),
    }).unwrap().packets;
    let item_index = packets.iter().find_map(|packet| match packet {
        ServerPacket::NPCGoods { list, .. } => list.iter()
            .find(|item| item.item_index == 658 && item.count == 1).map(|item| item.unique_id),
        _ => None,
    }).expect("real ordinary NPC catalogue");
    NpcGoldBuyRequest { item_index, count: 2, panel_type: 0 }
}

fn start(session: &mut GatewaySession) -> NpcGoldBuyRequest {
    login(session);
    session.execute_production_player_command(true, WorldCommand::ClientPacket(
        ClientPacket::StartGame { character_index: 0 },
    )).unwrap();
    open_catalog(session)
}

fn local_session() -> (GatewaySession, NpcGoldBuyRequest, Arc<InMemoryGameplayEventSink>) {
    let runtime = InProcessZoneRuntimeFactory.create_runtime(config(), &ZoneId::primary());
    let sink = Arc::new(InMemoryGameplayEventSink::default());
    let mut session = GatewaySession::with_routed_world_runtime_and_event_sink(
        ZoneId::primary(), runtime, sink.clone(),
    );
    session.zone_owner_command_client = Arc::new(InProcessZoneOwnerCommandClient::new());
    let request = start(&mut session);
    sink.drain();
    (session, request, sink)
}

fn shared_session() -> (GatewaySession, NpcGoldBuyRequest) {
    // Existing owner cadence is disabled for this finite fixture; no network surface.
    let factory = SharedInProcessZoneRuntimeFactory::new().fresh_replica();
    let registry = ZoneRegistry::new(ZoneId::primary(), Arc::new(factory));
    let mut session = GatewaySession::new_with_zone_registry(config(), &registry);
    session.zone_owner_command_client = Arc::new(InProcessZoneOwnerCommandClient::new());
    let request = start(&mut session);
    (session, request)
}

fn before(reason: NpcGoldBuyBeforeExecution) -> NpcGoldBuyRouteError {
    NpcGoldBuyRouteError::Processing(NpcGoldBuyProcessingError::BeforeExecution(reason))
}

fn delta(routed: &NpcGoldBuyRouteExecution, request: NpcGoldBuyRequest) -> UserItem {
    let NpcGoldBuyProcessingOutcome::Committed {
        request: actual, gold_spent, incoming_unique_id,
    } = &routed.processing_outcome else { panic!("actual commit required") };
    assert_eq!(*actual, request);
    assert_eq!(*gold_spent, 160);
    assert_ne!(*incoming_unique_id, 0);
    let packet = routed.execution.packets.iter().find_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item.clone()), _ => None,
    }).unwrap();
    assert_eq!(packet.unique_id, *incoming_unique_id);
    assert_eq!(packet.count, 2);
    assert_eq!(routed.execution.packets.iter()
        .filter(|packet| matches!(packet, ServerPacket::LoseGold { gold: 160 })).count(), 1);
    assert_eq!(routed.execution.outcome.command_kind, WorldCommandKind::ClientPacket("BuyItem"));
    assert_eq!(routed.execution.outcome.packet_count, routed.execution.packets.len());
    assert!(routed.execution.game_shop_purchase_outcome.is_none());
    packet
}

fn economic_snapshot(session: &GatewaySession) -> serde_json::Value {
    let snapshot = session.world_snapshot();
    serde_json::json!({ "gold": snapshot.gold, "bag": snapshot.inventory_items,
        "belt": snapshot.belt_items, "equipment": snapshot.equipment_items })
}

fn purchased_units(session: &GatewaySession) -> u32 {
    let snapshot = session.world_snapshot();
    snapshot.inventory_items.iter().chain(snapshot.belt_items.iter())
        .filter(|item| item.tooltip_source.as_ref().is_some_and(|source| source.info.item_index == 658))
        .map(|item| item.quantity).sum()
}

fn set_gold(session: &mut GatewaySession, gold: u32) -> NpcGoldBuyRequest {
    let mut checkpoint = session.runtime.active_character_checkpoint().unwrap();
    checkpoint.gold = gold;
    session.runtime.restore_active_character_checkpoint(&checkpoint).unwrap();
    open_catalog(session)
}

#[test]
fn npc_gold_gateway_actual_success_reject_success_has_current_outcome_metadata_and_events() {
    let (mut session, request, sink) = local_session();
    let initial_units = purchased_units(&session);
    assert!(session.supports_typed_npc_gold_buy_outcome());
    let first = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    let first_item = delta(&first, request);
    assert_eq!(session.world_snapshot().gold, 99_840);
    assert_eq!(purchased_units(&session), initial_units + 2);
    assert_eq!(first.execution.outcome.snapshot_tick, session.world_snapshot().tick);
    assert_eq!(first.execution.outcome.active_identity, session.active_identity());
    assert_eq!(session.active_identity_binding, session.active_identity());
    let low_request = set_gold(&mut session, 0);
    assert_eq!(low_request, request);
    let prior = economic_snapshot(&session);
    let rejected = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    assert_eq!(rejected.processing_outcome, NpcGoldBuyProcessingOutcome::Rejected {
        request, reason: NpcGoldBuyRejection::InsufficientGold,
    });
    assert_eq!(economic_snapshot(&session), prior);
    assert!(rejected.execution.packets.iter().all(|packet|
        !matches!(packet, ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })));
    assert_eq!(set_gold(&mut session, 1_000), request);
    let final_execution = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    let final_item = delta(&final_execution, request);
    assert_ne!(first_item.unique_id, final_item.unique_id);
    assert_eq!(session.world_snapshot().gold, 840);
    assert_eq!(purchased_units(&session), initial_units + 4);
    let events = sink.list().into_iter().filter(|event| event.command_kind == "client.BuyItem").collect::<Vec<_>>();
    assert_eq!(events.len(), 3);
    for (event, execution) in events.iter().zip([&first, &rejected, &final_execution]) {
        assert_eq!(event.packet_count, execution.execution.outcome.packet_count);
        assert_eq!(event.snapshot_tick, execution.execution.outcome.snapshot_tick);
        assert_eq!(event.account_id.as_deref(), Some("demo"));
        assert_eq!(event.character_index, Some(0));
    }
}

#[test]
fn npc_gold_gateway_false_unlogged_and_pre_start_are_guarded_without_economy_or_events() {
    let factory = SharedInProcessZoneRuntimeFactory::new().fresh_replica();
    let runtime = factory.create_runtime(config(), &ZoneId::primary());
    let sink = Arc::new(InMemoryGameplayEventSink::default());
    let mut session = GatewaySession::with_routed_world_runtime_and_event_sink(ZoneId::primary(), runtime, sink.clone());
    session.zone_owner_command_client = Arc::new(InProcessZoneOwnerCommandClient::new());
    let request = NpcGoldBuyRequest { item_index: 0, count: 2, panel_type: 0 };
    for authenticated in [false, true] {
        let old = economic_snapshot(&session);
        assert_eq!(session.execute_production_npc_gold_buy_requiring_typed_outcome(authenticated, request).unwrap_err(),
            before(NpcGoldBuyBeforeExecution::NotAuthenticated));
        assert_eq!(economic_snapshot(&session), old);
        assert_eq!(shared_test::economy_sequence(&mut session.runtime), 0);
        assert!(shared_test::context_is_clear(&mut session.runtime));
        assert!(sink.list().is_empty());
        assert!(session.active_identity_binding.is_none());
    }
    login(&mut session);
    sink.drain();
    let old = economic_snapshot(&session);
    assert_eq!(session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err(),
        before(NpcGoldBuyBeforeExecution::NotInGame));
    assert_eq!(economic_snapshot(&session), old);
    assert_eq!(shared_test::economy_sequence(&mut session.runtime), 0);
    assert!(sink.list().is_empty());
}

#[test]
fn npc_gold_shared_logout_returns_to_session_guard_without_context_or_sequence_advance() {
    let (mut session, request) = shared_session();
    assert!(session.active_identity().is_some());
    let logout = session.execute_production_player_command(true, WorldCommand::ClientPacket(ClientPacket::LogOut)).unwrap();
    assert!(logout.packets.iter().any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(session.active_identity().is_none());
    let sequence = shared_test::economy_sequence(&mut session.runtime);
    let prior = economic_snapshot(&session);
    let sink = Arc::new(InMemoryGameplayEventSink::default());
    session.gameplay_event_publisher = Some(GatewayGameplayEventPublisher::new(ZoneId::primary(), sink.clone()));
    assert_eq!(session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err(),
        before(NpcGoldBuyBeforeExecution::NotInGame));
    assert_eq!(economic_snapshot(&session), prior);
    assert_eq!(shared_test::economy_sequence(&mut session.runtime), sequence);
    assert!(shared_test::context_is_clear(&mut session.runtime));
    assert!(sink.list().is_empty());
    assert!(session.active_identity_binding.is_none());
}

#[derive(Debug)]
struct UnsupportedClient {
    generic_calls: AtomicUsize,
    advertised: bool,
}

#[derive(Debug, Default)]
struct DefaultUnsupportedClient { generic_calls: AtomicUsize }
impl ZoneOwnerCommandClient for DefaultUnsupportedClient {
    fn execute(&self, _runtime: &mut ZoneRuntimeHandle, _request: ZoneOwnerCommandRequest)
        -> Result<WorldCommandExecution, String> {
        self.generic_calls.fetch_add(1, Ordering::SeqCst);
        Err("forbidden default client generic fallback".into())
    }
}
impl ZoneOwnerCommandClient for UnsupportedClient {
    fn execute(&self, _runtime: &mut ZoneRuntimeHandle, _request: ZoneOwnerCommandRequest)
        -> Result<WorldCommandExecution, String> {
        self.generic_calls.fetch_add(1, Ordering::SeqCst);
        Err("forbidden generic fallback".into())
    }
    fn supports_typed_npc_gold_buy_outcome(&self, _runtime: &ZoneRuntimeHandle) -> bool { self.advertised }
}

#[test]
fn npc_gold_gateway_default_and_lying_custom_owner_are_unsupported_without_generic_execution() {
    let (mut default_session, request, sink) = local_session();
    let default_client = Arc::new(DefaultUnsupportedClient::default());
    default_session.zone_owner_command_client = default_client.clone();
    assert!(default_session.runtime.supports_typed_npc_gold_buy_outcome());
    assert!(!default_session.supports_typed_npc_gold_buy_outcome());
    assert_eq!(default_session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err(),
        before(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
    assert_eq!(default_client.generic_calls.load(Ordering::SeqCst), 0);
    assert!(sink.list().is_empty());
    for advertised in [false, true] {
        let (mut session, request, sink) = local_session();
        let client = Arc::new(UnsupportedClient { generic_calls: AtomicUsize::new(0), advertised });
        session.zone_owner_command_client = client.clone();
        let old = economic_snapshot(&session);
        assert_eq!(session.supports_typed_npc_gold_buy_outcome(), advertised);
        assert_eq!(session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err(),
            before(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
        assert_eq!(client.generic_calls.load(Ordering::SeqCst), 0);
        assert_eq!(economic_snapshot(&session), old);
        assert!(sink.list().is_empty());
    }
}

#[test]
fn npc_gold_gateway_stale_lease_stops_before_owner_client_and_runtime() {
    let (mut session, request, sink) = local_session();
    let client = Arc::new(UnsupportedClient { generic_calls: AtomicUsize::new(0), advertised: true });
    session.zone_owner_command_client = client.clone();
    let lease = session.zone_owner_lease.clone();
    let stale = ZoneOwnerLease::new(lease.zone_id().clone(), lease.owner_id(), lease.fencing_token() + 1);
    let old = economic_snapshot(&session);
    assert!(matches!(session.execute_production_npc_gold_buy_requiring_typed_outcome_with_zone_owner_lease(
        &stale, true, request,
    ), Err(NpcGoldBuyRouteError::BeforeExecution { .. })));
    assert_eq!(client.generic_calls.load(Ordering::SeqCst), 0);
    assert_eq!(economic_snapshot(&session), old);
    assert!(sink.list().is_empty());
}

#[derive(Clone, Copy)]
enum Fault { SnapshotPanic, IdentityPanic, BeforeCapturePanic, KnownPostError }

// Instrumentation delegates every economic result to a real InProcess Session.
struct ActualRuntimeProbe {
    inner: InProcessWorldRuntime,
    generic_calls: Arc<AtomicUsize>,
    typed_calls: Arc<AtomicUsize>,
    fault: Option<Fault>,
    metadata_fault: AtomicBool,
    fault_identity: bool,
}
impl WorldRuntime for ActualRuntimeProbe {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn on_connect(&self) -> Vec<ServerPacket> { self.inner.on_connect() }
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.generic_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(command)
    }
    fn supports_typed_npc_gold_buy_outcome(&self) -> bool { true }
    fn execute_production_npc_gold_buy_requiring_typed_outcome(&mut self, authenticated: bool, request: NpcGoldBuyRequest)
        -> Result<NpcGoldBuyProcessingExecution, NpcGoldBuyProcessingError> {
        self.typed_calls.fetch_add(1, Ordering::SeqCst);
        let fault = self.fault.take();
        if matches!(fault, Some(Fault::BeforeCapturePanic)) { panic!("controlled pre-capture failure"); }
        let processed = self.inner.execute_production_npc_gold_buy_requiring_typed_outcome(authenticated, request)?;
        match fault {
            Some(Fault::KnownPostError) => Err(NpcGoldBuyProcessingError::PostProcessing {
                outcome: processed.outcome, detail: "controlled owner postprocessing error".into(),
            }),
            Some(Fault::SnapshotPanic | Fault::IdentityPanic) => {
                self.fault_identity = matches!(fault, Some(Fault::IdentityPanic));
                self.metadata_fault.store(true, Ordering::SeqCst);
                Ok(processed)
            }
            _ => Ok(processed),
        }
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        if !self.fault_identity && self.metadata_fault.swap(false, Ordering::SeqCst) {
            panic!("controlled owner snapshot metadata panic");
        }
        self.inner.world_snapshot()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        if self.fault_identity && self.metadata_fault.swap(false, Ordering::SeqCst) {
            panic!("controlled owner identity metadata panic");
        }
        self.inner.active_identity()
    }
    fn active_character_checkpoint(&self) -> Option<mir2_simulation::CharacterSaveRecord> { self.inner.active_character_checkpoint() }
    fn save_active_character(&mut self) -> Result<(), String> { self.inner.save_active_character() }
    fn refresh_active_external_mail(&mut self) -> bool { false }
}

fn probe_session(fault: Option<Fault>) -> (GatewaySession, NpcGoldBuyRequest, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let (session, request, _) = local_session();
    let checkpoint = session.runtime.active_character_checkpoint().unwrap();
    let mut inner = InProcessWorldRuntime::new(config());
    inner.execute(WorldCommand::ClientPacket(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })).unwrap();
    inner.execute(WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 })).unwrap();
    inner.restore_active_character_checkpoint(&checkpoint).unwrap();
    inner.execute(WorldCommand::Interact { object_id: NPC_ID }).unwrap();
    inner.execute(WorldCommand::SelectNpcDialog { target: "@BuySell".into() }).unwrap();
    let generic = Arc::new(AtomicUsize::new(0));
    let typed = Arc::new(AtomicUsize::new(0));
    let runtime: ZoneRuntimeHandle = Box::new(ActualRuntimeProbe {
        inner, generic_calls: generic.clone(), typed_calls: typed.clone(), fault,
        metadata_fault: AtomicBool::new(false), fault_identity: false,
    });
    let mut gateway = GatewaySession::with_routed_world_runtime(ZoneId::primary(), runtime);
    gateway.zone_owner_command_client = Arc::new(InProcessZoneOwnerCommandClient::new());
    (gateway, request, generic, typed)
}

struct DefaultTypedRuntime {
    inner: InProcessWorldRuntime,
    advertised: bool,
    generic_calls: Arc<AtomicUsize>,
}
impl WorldRuntime for DefaultTypedRuntime {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn on_connect(&self) -> Vec<ServerPacket> { self.inner.on_connect() }
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.generic_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(command)
    }
    fn supports_typed_npc_gold_buy_outcome(&self) -> bool { self.advertised }
    // The trait's unsupported typed entry is deliberately not overridden.
    fn world_snapshot(&self) -> WorldSnapshot { self.inner.world_snapshot() }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> { self.inner.active_identity() }
    fn save_active_character(&mut self) -> Result<(), String> { self.inner.save_active_character() }
    fn refresh_active_external_mail(&mut self) -> bool { false }
}

#[test]
fn npc_gold_runtime_default_and_lying_support_never_call_legacy_purchase() {
    for advertised in [false, true] {
        let (mut session, request, generic, _) = probe_session(None);
        let probe = session.runtime.as_mut().as_any_mut().downcast_mut::<ActualRuntimeProbe>().unwrap();
        let inner = mem::replace(&mut probe.inner, InProcessWorldRuntime::new(config()));
        session.runtime = Box::new(DefaultTypedRuntime { inner, advertised, generic_calls: generic.clone() });
        let old = economic_snapshot(&session);
        assert_eq!(session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err(),
            before(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
        assert_eq!(generic.load(Ordering::SeqCst), 0);
        assert_eq!(economic_snapshot(&session), old);
    }
}

#[test]
fn npc_gold_in_process_owner_stale_lease_and_wrong_mode_or_tuple_never_enter_typed_runtime() {
    let (mut session, purchase, generic, typed) = probe_session(None);
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let lease = authority.owner_lease(&ZoneId::primary());
    let client = InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority.clone());
    let old = economic_snapshot(&session);
    let wrong_requests = [
        ZoneOwnerCommandRequest::direct(lease.clone(), npc_gold_buy_command(purchase)),
        ZoneOwnerCommandRequest::production_player(lease.clone(), true, WorldCommand::Tick),
        ZoneOwnerCommandRequest::production_player(lease.clone(), true, npc_gold_buy_command(NpcGoldBuyRequest { count: 1, ..purchase })),
    ];
    for request in wrong_requests {
        assert!(matches!(client.execute_production_npc_gold_buy_requiring_typed_outcome(&mut session.runtime, request, purchase),
            Err(NpcGoldBuyRouteError::BeforeExecution { .. })));
    }
    authority.handoff_zone_owner(&ZoneId::primary(), "replacement-owner");
    assert!(matches!(client.execute_production_npc_gold_buy_requiring_typed_outcome(&mut session.runtime,
        ZoneOwnerCommandRequest::production_player(lease, true, npc_gold_buy_command(purchase)), purchase,
    ), Err(NpcGoldBuyRouteError::BeforeExecution { .. })));
    assert_eq!(generic.load(Ordering::SeqCst), 0);
    assert_eq!(typed.load(Ordering::SeqCst), 0);
    assert_eq!(economic_snapshot(&session), old);
}

#[test]
fn npc_gold_rpc_and_hosted_owner_do_not_advertise_shadow_capability_or_execute_fallback() {
    for rpc in [false, true] {
        let (mut gateway, request, _, _) = probe_session(None);
        let (mut owner, _, owner_generic, owner_typed) = probe_session(None);
        let owner_runtime = mem::replace(&mut owner.runtime, Box::new(InProcessWorldRuntime::new(config())));
        let hosted = Arc::new(HostedZoneOwnerCommandClient::new(owner_runtime));
        gateway.zone_owner_command_client = if rpc {
            Arc::new(RpcZoneOwnerCommandClient::new(hosted.clone()))
        } else { hosted.clone() };
        assert!(gateway.runtime.supports_typed_npc_gold_buy_outcome());
        assert!(!gateway.supports_typed_npc_gold_buy_outcome());
        let old = economic_snapshot(&gateway);
        assert_eq!(gateway.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err(),
            before(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
        assert_eq!(owner_generic.load(Ordering::SeqCst), 0);
        assert_eq!(owner_typed.load(Ordering::SeqCst), 0);
        assert_eq!(economic_snapshot(&gateway), old);
        assert_eq!(ZoneOwnerCommandClient::execute_production_npc_gold_buy_requiring_typed_outcome(
            hosted.as_ref(), &mut gateway.runtime,
            ZoneOwnerCommandRequest::production_player(gateway.zone_owner_lease.clone(), true, npc_gold_buy_command(request)), request,
        ).unwrap_err(), before(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
        assert_eq!(ZoneOwnerCommandClient::execute_production_npc_gold_buy_requiring_typed_outcome(
            &RpcZoneOwnerCommandClient::new(hosted.clone()), &mut gateway.runtime,
            ZoneOwnerCommandRequest::production_player(gateway.zone_owner_lease.clone(), true, npc_gold_buy_command(request)), request,
        ).unwrap_err(), before(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
        assert_eq!(owner_generic.load(Ordering::SeqCst), 0);
        assert_eq!(owner_typed.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn npc_gold_owner_metadata_panic_and_known_post_error_preserve_actual_commit() {
    for fault in [Fault::SnapshotPanic, Fault::IdentityPanic, Fault::KnownPostError] {
        let (mut session, request, generic, typed) = probe_session(Some(fault));
        let error = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err();
        assert!(matches!(error, NpcGoldBuyRouteError::Processing(NpcGoldBuyProcessingError::PostProcessing {
            outcome: NpcGoldBuyProcessingOutcome::Committed { request: actual, gold_spent: 160, .. }, ..
        }) if actual == request));
        assert_eq!(session.world_snapshot().gold, 99_840);
        assert_eq!(generic.load(Ordering::SeqCst), 0);
        assert_eq!(typed.load(Ordering::SeqCst), 1);
        let next = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
        delta(&next, request);
        assert_eq!(session.world_snapshot().gold, 99_680);
    }
}

#[test]
fn npc_gold_owner_before_capture_panic_is_unknown_and_next_call_has_its_own_outcome() {
    let (mut session, request, generic, typed) = probe_session(Some(Fault::BeforeCapturePanic));
    let old = economic_snapshot(&session);
    assert!(matches!(session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request),
        Err(NpcGoldBuyRouteError::Processing(NpcGoldBuyProcessingError::Unknown { .. }))));
    assert_eq!(economic_snapshot(&session), old);
    let next = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    delta(&next, request);
    assert_eq!(generic.load(Ordering::SeqCst), 0);
    assert_eq!(typed.load(Ordering::SeqCst), 2);
}

#[test]
fn npc_gold_shared_real_purchase_retains_predrain_and_full_tail_packets() {
    let (mut session, request) = shared_session();
    let initial_sequence = shared_test::economy_sequence(&mut session.runtime);
    let pending = ServerPacket::KeepAlive { time: 7441 };
    shared_test::queue_pending(&mut session.runtime, pending.clone());
    shared_test::require_tail_after_capture(&mut session.runtime);
    let result = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    delta(&result, request);
    assert!(result.execution.packets.contains(&pending), "real shared predrain is retained");
    assert!(shared_test::context_is_clear(&mut session.runtime));
    assert!(shared_test::capture_hook_and_tail_consumed(&mut session.runtime), "actual capture hook and sync_zone_snapshot tail both ran");
    assert_eq!(shared_test::economy_sequence(&mut session.runtime), initial_sequence + 1);
    assert_eq!(result.execution.outcome.active_identity, session.active_identity());
    assert_eq!(result.execution.outcome.snapshot_tick, session.world_snapshot().tick);
    assert_eq!(session.world_snapshot().gold, 99_840);
    let rejected_request = NpcGoldBuyRequest { count: 0, ..request };
    let old = economic_snapshot(&session);
    let rejected = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, rejected_request).unwrap();
    assert_eq!(rejected.processing_outcome, NpcGoldBuyProcessingOutcome::Rejected {
        request: rejected_request, reason: NpcGoldBuyRejection::InvalidRequest,
    });
    assert_eq!(economic_snapshot(&session), old);
    assert_eq!(shared_test::economy_sequence(&mut session.runtime), initial_sequence + 2);
    let next = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    assert_ne!(delta(&next, request).unique_id, delta(&result, request).unique_id);
    assert_eq!(shared_test::economy_sequence(&mut session.runtime), initial_sequence + 3);
}

#[test]
fn npc_gold_shared_post_capture_reconcile_unwind_keeps_commit_and_clears_context() {
    let (mut session, request) = shared_session();
    shared_test::poison_after_capture(&mut session.runtime);
    let result = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request);
    // Clear this fixture's deliberate poison before any assertion, snapshot or Drop.
    shared_test::clear_poison(&mut session.runtime);
    assert!(matches!(result, Err(NpcGoldBuyRouteError::Processing(NpcGoldBuyProcessingError::PostProcessing {
        outcome: NpcGoldBuyProcessingOutcome::Committed { request: actual, gold_spent: 160, .. }, ..
    })) if actual == request));
    assert!(shared_test::context_is_clear(&mut session.runtime));
    assert_eq!(session.world_snapshot().gold, 99_840);
    let next = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    delta(&next, request);
    assert_eq!(session.world_snapshot().gold, 99_680);
}

#[test]
fn npc_gold_shared_pre_capture_failure_is_unknown_after_previous_success() {
    let (mut session, request) = shared_session();
    let first = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    let first_item = delta(&first, request);
    shared_test::poison_before_capture(&mut session.runtime);
    let result = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request);
    shared_test::clear_poison(&mut session.runtime);
    assert!(matches!(result, Err(NpcGoldBuyRouteError::Processing(NpcGoldBuyProcessingError::Unknown { .. }))));
    assert!(shared_test::context_is_clear(&mut session.runtime));
    assert_eq!(session.world_snapshot().gold, 99_840);
    let next = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    assert_ne!(delta(&next, request).unique_id, first_item.unique_id);
}

struct PanicSink;
impl GameplayEventSink for PanicSink {
    fn publish(&self, _event: GatewayGameplayEvent) { panic!("controlled gateway event failure"); }
    fn status(&self) -> GameplayEventSinkStatus { GameplayEventSinkStatus::not_configured() }
}

#[test]
fn npc_gold_gateway_event_panic_retains_actual_commit_and_subsequent_rejection() {
    let (mut session, request, _) = local_session();
    session.gameplay_event_publisher = Some(GatewayGameplayEventPublisher::new(ZoneId::primary(), Arc::new(PanicSink)));
    let error = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap_err();
    session.gameplay_event_publisher = None; // poisoned publisher belongs only to this fixture
    assert!(matches!(error, NpcGoldBuyRouteError::Processing(NpcGoldBuyProcessingError::PostProcessing {
        outcome: NpcGoldBuyProcessingOutcome::Committed { request: actual, gold_spent: 160, .. }, ..
    }) if actual == request));
    assert_eq!(session.world_snapshot().gold, 99_840);
    assert_eq!(set_gold(&mut session, 0), request);
    let next = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    assert_eq!(next.processing_outcome, NpcGoldBuyProcessingOutcome::Rejected {
        request, reason: NpcGoldBuyRejection::InsufficientGold,
    });
}

#[test]
fn npc_gold_gateway_global_bus_drain_preserves_existing_event_and_packet_count_order() {
    let (mut session, request, sink) = local_session();
    let bus = Arc::new(GlobalZoneMessageBus::default());
    bus.register_session(&session.session_id, session.zone_id.clone());
    bus.update_session(&session.session_id, session.zone_id.clone(), true);
    let pending = ServerPacket::KeepAlive { time: 7422 };
    bus.publish_to_other_zones("source", &ZoneId::new("other-zone"), &[pending.clone()]);
    session.global_message_bus = Some(bus.clone());
    let result = session.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    delta(&result, request);
    assert_eq!(result.execution.packets.last(), Some(&pending));
    assert!(bus.drain(&session.session_id).is_empty());
    assert_eq!(sink.list().last().unwrap().packet_count + 1, result.execution.outcome.packet_count);
    assert_eq!(session.active_identity_binding, result.execution.outcome.active_identity);
}

#[test]
fn npc_gold_explicit_route_leaves_legacy_npc_and_game_shop_paths_unchanged() {
    let (mut typed, request, _) = local_session();
    let (mut legacy, legacy_request, _) = local_session();
    let typed_result = typed.execute_production_npc_gold_buy_requiring_typed_outcome(true, request).unwrap();
    let mut new_item = delta(&typed_result, request);
    let old_result = legacy.execute_production_player_command(true, npc_gold_buy_command(legacy_request)).unwrap();
    assert!(old_result.game_shop_purchase_outcome.is_none());
    let mut old_item = old_result.packets.iter().find_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item.clone()), _ => None,
    }).unwrap();
    new_item.unique_id = 0;
    old_item.unique_id = 0;
    assert_eq!(new_item, old_item);
    assert_eq!(typed.world_snapshot().gold, legacy.world_snapshot().gold);
    let game_shop = legacy.execute_production_player_command(true, WorldCommand::ClientPacket(ClientPacket::GameShopBuy {
        g_index: i32::MAX, quantity: 1, price_type: 0,
    })).unwrap();
    assert!(game_shop.game_shop_purchase_outcome.is_some());
}
