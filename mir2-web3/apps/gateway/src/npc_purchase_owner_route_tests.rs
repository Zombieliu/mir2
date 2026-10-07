//! Actual File-source owner routes and in-library wire/host boundaries.
//! No listener, socket call, autonomous Zone tick, environment change or process
//! restart is exercised. Owned TEMP files retain the finite fixture evidence.

use super::*;
use crate::routing::{InMemoryZoneOwnerLeaseAuthority, InProcessZoneOwnerCommandClient,
    RpcZoneOwnerCommandClient, ZoneOwnerCommandClient, ZoneOwnerLeaseAuthority};
use mir2_protocol::MirDirection;
use mir2_simulation::npc_purchase_journal::NpcPurchaseOperation;
use mir2_simulation::{AccountStore, InProcessWorldRuntime, NpcPurchaseCurrency,
    NpcPurchaseProcessingOutcome,
    NpcPurchaseReceipt, NpcPurchaseRequest, NpcPurchaseSource, VisibleNpcRecord,
    ZoneRuntimeHandle};
use std::fs;
use std::path::PathBuf;

const NPC: u32 = 4990;
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct FileOwner {
    config: GatewayConfig,
    path: PathBuf,
    runtime: ZoneRuntimeHandle,
    client: Arc<dyn ZoneOwnerCommandClient>,
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    lease: ZoneOwnerLease,
    request: NpcPurchaseRequest,
}

fn owned_config(label: &str) -> (GatewayConfig, PathBuf) {
    let temporary = std::env::temp_dir().canonicalize().expect("guarded runner TEMP must exist");
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = temporary.join(format!("mir2-source28-owner-{label}-{timestamp}-{sequence}"));
    assert!(directory.starts_with(&temporary));
    fs::create_dir(&directory).expect("fresh owned fixture directory");
    let path = directory.join("accounts.json");
    let mut config = GatewayConfig::default();
    assert!(config.account_store_path.is_none());
    assert!(config.account_store_database_url.is_none());
    config.save_recovery_dir = None;
    config.visible_monsters.clear();
    config.map_hazards.clear();
    config.visible_npcs.push(VisibleNpcRecord { object_id: NPC, name: "Wicked Trader".into(),
        image: 5, colour_argb: -1, position: Point { x: config.spawn.x + 1, y: config.spawn.y },
        direction: MirDirection::Left, quest_ids: vec![],
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()) });
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
    // Seed only the fresh owned File, then use its real source authority and
    // ordinary Session save path. Never edit a loaded private repository.
    let bytes = serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap();
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    file.write_all(&bytes).unwrap();
    drop(file);
    config = config.with_account_store_path(&path);
    assert!(config.account_store_database_url.is_none());
    (config, path)
}

fn bootstrap(runtime: &mut ZoneRuntimeHandle) -> NpcPurchaseRequest {
    let login = runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    })).unwrap();
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 })).unwrap();
    assert_eq!(runtime.active_identity().unwrap().character_index, 0);
    runtime.execute(WorldCommand::Interact { object_id: NPC }).unwrap();
    let goods = runtime.execute(WorldCommand::SelectNpcDialog { target: "@BuySell".into() }).unwrap();
    let item_index = goods.iter().find_map(|packet| match packet {
        ServerPacket::NPCGoods { list, .. } => list.iter().find(|item| item.item_index == 658 && item.count == 1)
            .map(|item| item.unique_id),
        _ => None,
    }).expect("actual WickedTrader selector");
    runtime.save_active_character().unwrap();
    NpcPurchaseRequest { item_index, count: 2, panel_type: 0 }
}

impl FileOwner {
    fn new(label: &str, hosted_rpc_adapter: bool) -> Self {
        let (config, path) = owned_config(label);
        let mut runtime: ZoneRuntimeHandle = Box::new(InProcessWorldRuntime::new(config.clone()));
        let request = bootstrap(&mut runtime);
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let lease = authority.owner_lease(&ZoneId::primary());
        let client: Arc<dyn ZoneOwnerCommandClient> = if hosted_rpc_adapter {
            let host = Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(runtime, authority.clone()));
            let memory = GatewayConfig::default();
            assert!(memory.account_store_path.is_none());
            assert!(memory.account_store_database_url.is_none());
            runtime = Box::new(InProcessWorldRuntime::new(memory));
            Arc::new(RpcZoneOwnerCommandClient::new(host))
        } else {
            Arc::new(InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority.clone()))
        };
        Self { config, path, runtime, client, authority, lease, request }
    }

    fn execute(&mut self, action: NpcPurchaseOwnerAction) -> Result<NpcPurchaseOwnerRouteExecution, NpcPurchaseDurableError> {
        self.client.execute_npc_purchase_owner(&mut self.runtime, &self.lease, true, action)
    }

    fn snapshot(&self) -> WorldSnapshot { self.client.world_snapshot(&self.runtime).unwrap() }

    fn save(&self) -> CharacterSaveRecord {
        let store: AccountStore = serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap();
        store.accounts["demo"].saves[&0].clone()
    }

    fn operation(&mut self, sequence: u64) -> NpcPurchaseOperation {
        let began = self.execute(NpcPurchaseOwnerAction::Begin).unwrap();
        let NpcPurchaseOwnerReply::Producer { producer } = began.reply else { panic!("actual producer required") };
        assert_eq!(began.authority, Some(producer));
        assert!(began.snapshot.is_some());
        let quoted = self.execute(NpcPurchaseOwnerAction::Quote { request: self.request }).unwrap();
        let NpcPurchaseOwnerReply::Quote { intent } = quoted.reply else { panic!("actual catalog intent required") };
        assert_eq!(intent.request, self.request);
        NpcPurchaseOperation { actor: producer.actor, request_scope: producer.producer_scope, sequence, intent }
    }
}

fn snapshot_value(snapshot: &WorldSnapshot) -> serde_json::Value { serde_json::to_value(snapshot).unwrap() }

fn assert_preflight(result: Result<NpcPurchaseOwnerRouteExecution, NpcPurchaseDurableError>) {
    assert!(matches!(result, Err(NpcPurchaseDurableError::BeforeExecution { .. })));
}

fn actual_commit(route: &NpcPurchaseOwnerRouteExecution, operation: NpcPurchaseOperation) -> NpcPurchaseReceipt {
    let NpcPurchaseOwnerReply::Purchase { receipt, replayed } = &route.reply else { panic!("genuine terminal purchase required") };
    assert!(!replayed);
    let incoming = route.execution.packets.iter().find_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item), _ => None,
    }).expect("actual delivery packet");
    assert_eq!(receipt.entry.operation, operation);
    assert_eq!(receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed {
        request: operation.intent.request, currency: NpcPurchaseCurrency::Gold, source: NpcPurchaseSource::Trade,
        charged: 160, admitted_count: 2, incoming_unique_id: incoming.unique_id,
    });
    assert_eq!(incoming.count, 2);
    assert_eq!(route.execution.packets.iter().filter(|packet| matches!(packet, ServerPacket::LoseGold { gold: 160 })).count(), 1);
    let authority = route.authority.unwrap();
    assert_eq!(authority.actor, operation.actor);
    assert_eq!(authority.producer_scope, receipt.producer_scope);
    assert_eq!(authority.server_revision, receipt.entry.server_revision);
    let snapshot = route.snapshot.as_ref().expect("actual complete authority pair");
    assert_eq!(snapshot.gold, 99_840);
    assert_eq!(route.execution.outcome.snapshot_tick, snapshot.tick);
    assert_eq!(route.execution.outcome.packet_count, route.execution.packets.len());
    assert_eq!(route.execution.outcome.command_kind, crate::npc_purchase_owner_route::action_kind(NpcPurchaseOwnerAction::Purchase { operation }));
    receipt.clone()
}

#[derive(Debug, Default)]
struct UnsupportedRouteProbe { generic_calls: AtomicUsize }

impl ZoneOwnerCommandClient for UnsupportedRouteProbe {
    fn execute(&self, _runtime: &mut ZoneRuntimeHandle, _request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.generic_calls.fetch_add(1, Ordering::Relaxed);
        Err("unexpected generic owner execution".into())
    }
}
impl ZoneOwnerRpcTransport for UnsupportedRouteProbe {
    fn execute(&self, _request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.generic_calls.fetch_add(1, Ordering::Relaxed);
        Err("unexpected generic RPC execution".into())
    }
    fn world_snapshot(&self) -> Result<WorldSnapshot, String> { Err("unsupported snapshot".into()) }
    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> { Err("unsupported identity".into()) }
    fn save_active_character(&self) -> Result<(), String> { Err("unsupported save".into()) }
    fn refresh_active_external_mail(&self) -> Result<bool, String> { Err("unsupported refresh".into()) }
}

#[test]
fn durable_owner_defaults_and_missing_lease_authority_refuse_without_generic_execution() {
    let mut fixture = FileOwner::new("default-refusal", false);
    let operation = fixture.operation(1);
    let bytes = fs::read(&fixture.path).unwrap();
    let before = snapshot_value(&fixture.snapshot());
    let default = UnsupportedRouteProbe::default();
    let unguarded = InProcessZoneOwnerCommandClient::new();
    assert!(!ZoneOwnerCommandClient::supports_durable_npc_purchase_owner(&default, &fixture.runtime));
    assert!(!ZoneOwnerRpcTransport::supports_durable_npc_purchase_owner(&default));
    assert!(!unguarded.supports_durable_npc_purchase_owner(&fixture.runtime));
    for action in [NpcPurchaseOwnerAction::Begin, NpcPurchaseOwnerAction::Purchase { operation }] {
        assert_preflight(ZoneOwnerCommandClient::execute_npc_purchase_owner(&default, &mut fixture.runtime, &fixture.lease, true, action));
        assert_preflight(ZoneOwnerRpcTransport::execute_npc_purchase_owner(&default, &fixture.lease, true, action));
        assert_preflight(unguarded.execute_npc_purchase_owner(&mut fixture.runtime, &fixture.lease, true, action));
    }
    assert_eq!(default.generic_calls.load(Ordering::Relaxed), 0);
    assert_eq!(snapshot_value(&fixture.snapshot()), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    let unguarded_host = HostedZoneOwnerCommandClient::new(fixture.runtime);
    assert!(!unguarded_host.supports_durable_npc_purchase_owner());
    assert_preflight(unguarded_host.execute_npc_purchase_owner(&fixture.lease, true, NpcPurchaseOwnerAction::Begin));
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn durable_in_process_begin_quote_and_query_use_actual_file_and_read_only_original_id() {
    let mut fixture = FileOwner::new("in-process-read", false);
    assert!(fixture.client.supports_durable_npc_purchase_owner(&fixture.runtime));
    let operation = fixture.operation(1);
    let before = snapshot_value(&fixture.snapshot());
    let bytes = fs::read(&fixture.path).unwrap();
    let baseline = fixture.save();
    for _ in 0..2 {
        let begin = fixture.execute(NpcPurchaseOwnerAction::Begin).unwrap();
        let NpcPurchaseOwnerReply::Producer { producer } = begin.reply else { panic!("actual Begin") };
        assert_eq!(producer.actor, operation.actor);
        assert_eq!(producer.producer_scope, operation.request_scope);
        assert_eq!(producer.server_revision, baseline.revision);
        assert_eq!(begin.authority, Some(producer));
        let quote = fixture.execute(NpcPurchaseOwnerAction::Quote { request: fixture.request }).unwrap();
        assert_eq!(quote.reply, NpcPurchaseOwnerReply::Quote { intent: operation.intent });
        let query = fixture.execute(NpcPurchaseOwnerAction::Query { operation }).unwrap();
        assert_eq!(query.reply, NpcPurchaseOwnerReply::Recovery { receipt: None });
        for route in [quote, query] {
            assert!(route.execution.packets.is_empty());
            assert_eq!(route.authority, Some(producer));
            assert_eq!(snapshot_value(route.snapshot.as_ref().unwrap()), before);
        }
        assert_eq!(snapshot_value(&fixture.snapshot()), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    }
    assert!(fixture.save().npc_purchase_journal.unwrap().entries.is_empty());
    assert_preflight(fixture.client.execute_npc_purchase_owner(&mut fixture.runtime, &fixture.lease, false, NpcPurchaseOwnerAction::Begin));
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn durable_hosted_rpc_adapter_uses_real_file_owner_and_leaves_unlogged_shadow_untouched() {
    let mut fixture = FileOwner::new("hosted-adapter-read", true);
    assert!(fixture.client.supports_durable_npc_purchase_owner(&fixture.runtime));
    assert!(!fixture.runtime.supports_durable_npc_purchase_owner());
    assert!(fixture.runtime.active_identity().is_none());
    let shadow = snapshot_value(&fixture.runtime.world_snapshot());
    let operation = fixture.operation(1);
    let bytes = fs::read(&fixture.path).unwrap();
    let before = snapshot_value(&fixture.snapshot());
    let quoted = fixture.execute(NpcPurchaseOwnerAction::Quote { request: fixture.request }).unwrap();
    assert_eq!(quoted.reply, NpcPurchaseOwnerReply::Quote { intent: operation.intent });
    let query = fixture.execute(NpcPurchaseOwnerAction::Query { operation }).unwrap();
    assert_eq!(query.reply, NpcPurchaseOwnerReply::Recovery { receipt: None });
    assert_eq!(query.authority.unwrap().actor, fixture.save().npc_purchase_journal.unwrap().actor);
    assert!(query.execution.packets.is_empty());
    assert_eq!(snapshot_value(&fixture.snapshot()), before);
    assert_eq!(snapshot_value(&fixture.runtime.world_snapshot()), shadow);
    assert!(fixture.runtime.active_identity().is_none());
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn durable_actual_local_and_hosted_purchase_replay_preserve_receipt_checkpoint_and_authority() {
    for hosted in [false, true] {
        let mut fixture = FileOwner::new("purchase-replay", hosted);
        let shadow = hosted.then(|| snapshot_value(&fixture.runtime.world_snapshot()));
        let operation = fixture.operation(1);
        let baseline = fixture.save();
        let routed = fixture.execute(NpcPurchaseOwnerAction::Purchase { operation }).unwrap();
        let receipt = actual_commit(&routed, operation);
        let saved = fixture.save();
        assert_eq!(saved.revision, baseline.revision + 1);
        assert_eq!(saved.revision, receipt.entry.server_revision);
        assert_eq!(saved.gold, 99_840);
        assert_eq!(saved.npc_purchase_journal.as_ref().unwrap().entries, vec![receipt.entry.clone()]);
        assert_eq!(serde_json::to_value(routed.checkpoint.as_ref().unwrap()).unwrap(), serde_json::to_value(&saved).unwrap());
        let before = snapshot_value(&fixture.snapshot());
        let bytes = fs::read(&fixture.path).unwrap();
        let replay = fixture.execute(NpcPurchaseOwnerAction::Purchase { operation }).unwrap();
        assert_eq!(replay.reply, NpcPurchaseOwnerReply::Purchase { receipt: receipt.clone(), replayed: true });
        assert!(replay.execution.packets.is_empty());
        assert_eq!(replay.authority, routed.authority);
        assert_eq!(snapshot_value(replay.snapshot.as_ref().unwrap()), before);
        let query = fixture.execute(NpcPurchaseOwnerAction::Query { operation }).unwrap();
        assert_eq!(query.reply, NpcPurchaseOwnerReply::Recovery { receipt: Some(receipt) });
        assert_eq!(query.authority, routed.authority);
        assert!(query.execution.packets.is_empty());
        assert_eq!(snapshot_value(&fixture.snapshot()), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        if let Some(shadow) = shadow { assert_eq!(snapshot_value(&fixture.runtime.world_snapshot()), shadow); }
    }
}

#[test]
fn durable_lease_handoff_refuses_stale_debit_and_recovers_exact_original_operation() {
    let mut fixture = FileOwner::new("lease-handoff", true);
    let operation = fixture.operation(1);
    let actual = fixture.execute(NpcPurchaseOwnerAction::Purchase { operation }).unwrap();
    let receipt = actual_commit(&actual, operation);
    let old = fixture.lease.clone();
    let current = fixture.authority.handoff_zone_owner(old.zone_id(), "source28-next-owner");
    assert_ne!(current.fencing_token(), old.fencing_token());
    let before = snapshot_value(&fixture.snapshot());
    let bytes = fs::read(&fixture.path).unwrap();
    for action in [NpcPurchaseOwnerAction::Begin, NpcPurchaseOwnerAction::Query { operation }, NpcPurchaseOwnerAction::Purchase { operation }] {
        assert_preflight(fixture.client.execute_npc_purchase_owner(&mut fixture.runtime, &old, true, action));
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    }
    fixture.lease = current;
    assert_preflight(fixture.execute(NpcPurchaseOwnerAction::Query { operation }));
    let began = fixture.execute(NpcPurchaseOwnerAction::Begin).unwrap();
    let NpcPurchaseOwnerReply::Producer { producer } = began.reply else { panic!("current owner must Begin") };
    assert_eq!(producer.actor, operation.actor);
    assert_ne!(producer.producer_scope, operation.request_scope);
    assert_eq!(producer.server_revision, receipt.entry.server_revision);
    let query = fixture.execute(NpcPurchaseOwnerAction::Query { operation }).unwrap();
    let NpcPurchaseOwnerReply::Recovery { receipt: Some(recovered) } = query.reply else { panic!("original committed operation must recover") };
    assert_eq!(recovered.entry, receipt.entry);
    assert_eq!(recovered.producer_scope, producer.producer_scope);
    assert_eq!(query.authority, Some(producer));
    assert!(query.execution.packets.is_empty());
    assert_preflight(fixture.execute(NpcPurchaseOwnerAction::Purchase { operation }));
    assert_eq!(snapshot_value(&fixture.snapshot()), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn durable_real_receipt_wire_roundtrips_and_refuses_mismatched_action_pair_and_packet_count() {
    let mut fixture = FileOwner::new("wire-roundtrip", true);
    let operation = fixture.operation(1);
    let action = NpcPurchaseOwnerAction::Purchase { operation };
    let actual = fixture.execute(action).unwrap();
    let receipt = actual_commit(&actual, operation);
    let wire = WireNpcPurchaseOwnerResponse::from_execution(actual.clone());
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        let bytes = encode_rpc_value(&wire, codec).unwrap();
        let decoded: WireNpcPurchaseOwnerResponse = decode_rpc_value(&bytes, codec).unwrap();
        let route = decoded.into_execution(action).unwrap();
        assert_eq!(route.reply, actual.reply);
        assert_eq!(route.authority, actual.authority);
        assert_eq!(snapshot_value(route.snapshot.as_ref().unwrap()), snapshot_value(actual.snapshot.as_ref().unwrap()));
        assert_eq!(route.execution.outcome.packet_count, actual.execution.outcome.packet_count);
        assert_eq!(route.execution.outcome.active_identity, actual.execution.outcome.active_identity);
        assert_eq!(encode_server_frames(route.execution.packets).unwrap(), encode_server_frames(actual.execution.packets.clone()).unwrap());
    }
    let original = serde_json::to_value(&wire).unwrap();
    for alteration in ["authority", "snapshot", "actor", "revision", "packetCount"] {
        let mut value = original.clone();
        match alteration {
            "authority" => value["authority"] = serde_json::Value::Null,
            "snapshot" => value["snapshot"] = serde_json::Value::Null,
            "actor" => value["authority"]["actor"] = serde_json::json!([0; 32].to_vec()),
            "revision" => value["authority"]["serverRevision"] = serde_json::json!(receipt.entry.server_revision - 1),
            "packetCount" => value["packetCount"] = serde_json::json!(actual.execution.packets.len() + 1),
            _ => unreachable!(),
        }
        let malformed: WireNpcPurchaseOwnerResponse = serde_json::from_value(value).unwrap();
        let error = malformed.into_execution(action).unwrap_err();
        let NpcPurchaseDurableError::PostCommit { receipt: retained, .. } = error else { panic!("known real receipt must survive malformed {alteration}") };
        assert_eq!(retained, receipt);
    }
    let decoded: WireNpcPurchaseOwnerResponse = serde_json::from_value(original.clone()).unwrap();
    assert!(matches!(decoded.into_execution(NpcPurchaseOwnerAction::Begin), Err(NpcPurchaseDurableError::Unknown { .. })));
    let changed = NpcPurchaseOperation { sequence: operation.sequence + 1, ..operation };
    let decoded: WireNpcPurchaseOwnerResponse = serde_json::from_value(original).unwrap();
    assert!(matches!(decoded.into_execution(NpcPurchaseOwnerAction::Purchase { operation: changed }), Err(NpcPurchaseDurableError::Unknown { .. })));
    let request = WireNpcPurchaseOwnerRequest { protocol_version: NPC_PURCHASE_OWNER_PROTOCOL_VERSION,
        owner_lease: (&fixture.lease).into(), authenticated: true, action };
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        let decoded: WireNpcPurchaseOwnerRequest = decode_rpc_value(&encode_rpc_value(&request, codec).unwrap(), codec).unwrap();
        assert_eq!(decoded.action, action);
        assert_eq!(decoded.owner_lease.into_lease().unwrap(), fixture.lease);
    }
    let mut unknown = serde_json::to_value(&request).unwrap();
    unknown["fallback"] = serde_json::json!(true);
    assert!(serde_json::from_str::<WireNpcPurchaseOwnerRequest>(&unknown.to_string()).is_err());
    let canonical_request = serde_json::to_string(&request).unwrap();
    for spelling in ["protocolVersion", r"protocol\u0056ersion"] {
        let duplicate = canonical_request.replacen("\"protocolVersion\":",
            &format!("\"{spelling}\":1,\"protocolVersion\":"), 1);
        assert!(serde_json::from_str::<WireNpcPurchaseOwnerRequest>(&duplicate).is_err());
    }
    // Corrupt copies of this genuine routed terminal receipt. A semantically
    // illegal or unrelated receipt must remain Unknown, even in an Error arm;
    // it must never become known economic evidence merely because serde parsed.
    let good_receipt = serde_json::to_value(&receipt).unwrap();
    let correlated_error = WireNpcPurchaseOwnerResponse::Error { error: NpcPurchaseDurableError::PostCommit {
        receipt: receipt.clone(), detail: "wire derivative with the genuine correlated terminal result".into(),
    } };
    let NpcPurchaseDurableError::PostCommit { receipt: retained, .. } = correlated_error.into_execution(action).unwrap_err() else {
        panic!("valid correlated terminal evidence must remain recoverable");
    };
    assert_eq!(retained, receipt);
    for (pointer, replacement) in [
        ("/entry/operation/sequence", serde_json::json!(operation.sequence + 1)),
        ("/entry/outcome/committed/request/count", serde_json::json!(operation.intent.request.count + 1)),
        ("/entry/outcome/committed/currency", serde_json::json!("pearls")),
        ("/entry/outcome/committed/source", serde_json::json!("used")),
        ("/entry/outcome/committed/admittedCount", serde_json::json!(0)),
        ("/entry/outcome/committed/admittedCount", serde_json::json!(operation.intent.request.count + 1)),
        ("/entry/serverRevision", serde_json::json!(0)),
        ("/entry/serverRevision", serde_json::json!(u64::MAX)),
    ] {
        let mut bad_receipt = good_receipt.clone();
        *bad_receipt.pointer_mut(pointer).unwrap() = replacement;
        let mut execution = serde_json::to_value(&wire).unwrap();
        execution["reply"]["receipt"] = bad_receipt.clone();
        let decoded: WireNpcPurchaseOwnerResponse = serde_json::from_value(execution).unwrap();
        assert!(matches!(decoded.into_execution(action), Err(NpcPurchaseDurableError::Unknown { .. })), "execution {pointer}");
        let error = serde_json::json!({ "kind": "error", "error": {
            "kind": "postCommit", "receipt": bad_receipt, "detail": "malformed derivative of real terminal receipt"
        } });
        let decoded: WireNpcPurchaseOwnerResponse = serde_json::from_value(error).unwrap();
        assert!(matches!(decoded.into_execution(action), Err(NpcPurchaseDurableError::Unknown { .. })), "error {pointer}");
    }
}

#[test]
fn durable_mutation_classification_blocks_generic_tcp_fallback_before_any_endpoint_access() {
    let mut fixture = FileOwner::new("generic-fallback", false);
    let operation = fixture.operation(1);
    // Construct the real transport without env-selected pools or constructors;
    // the exercised generic executor must reject before any network access.
    let transport = TcpZoneOwnerRpcTransport {
        addresses: Arc::new(vec!["source28-no-endpoint".into()]),
        active_endpoint: Arc::new(AtomicUsize::new(0)), zone_id: ZoneId::primary(),
        session_id: "source28-generic-refusal".into(), auth_token: None,
        limits: ZoneRpcLimits::default(), codec: ZoneRpcCodec::Json, reuse_connections: false,
        connections: Arc::new(vec![Mutex::new(None)]), shared_connections: None,
        outbound_acknowledged: Arc::new(AtomicU64::new(0)), outbound_generation: Arc::new(AtomicU64::new(0)),
        outbound_stream_id: Arc::new(Mutex::new(None)),
    };
    assert!(!NpcPurchaseOwnerAction::Quote { request: fixture.request }.is_mutation());
    assert!(!NpcPurchaseOwnerAction::Query { operation }.is_mutation());
    for action in [NpcPurchaseOwnerAction::Quote { request: fixture.request }, NpcPurchaseOwnerAction::Query { operation }] {
        let request = ZoneRpcRequest::NpcPurchaseOwner { request: WireNpcPurchaseOwnerRequest {
            protocol_version: NPC_PURCHASE_OWNER_PROTOCOL_VERSION, owner_lease: (&fixture.lease).into(), authenticated: true, action,
        }};
        assert!(zone_rpc_request_requires_active_session(&request));
        assert!(!zone_rpc_request_requires_active_mutation(&request));
    }
    for action in [NpcPurchaseOwnerAction::Begin, NpcPurchaseOwnerAction::Purchase { operation }] {
        assert!(action.is_mutation());
        let request = ZoneRpcRequest::NpcPurchaseOwner { request: WireNpcPurchaseOwnerRequest {
            protocol_version: NPC_PURCHASE_OWNER_PROTOCOL_VERSION, owner_lease: (&fixture.lease).into(), authenticated: true, action,
        }};
        assert!(zone_rpc_request_requires_active_session(&request));
        assert!(zone_rpc_request_requires_active_mutation(&request));
        let error = transport.call(request).unwrap_err();
        assert!(error.contains("single-attempt executor"));
        assert!(transport.connections[0].lock().unwrap().is_none());
        assert_eq!(transport.active_endpoint.load(Ordering::Relaxed), 0);
    }
}

const HOST_TOKEN: &str = "source28-owned-host-test-token";
const HOST_SESSION: &str = "source28-owned-host-session";

fn host_envelope(request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope { protocol_version: ZONE_RPC_PROTOCOL_VERSION, session_id: HOST_SESSION.into(),
        zone_id: ZoneId::primary().as_str().into(), auth_token: Some(HOST_TOKEN.into()), request }
}

fn host_command(server: &ZoneHostServer, lease: &ZoneOwnerLease, command: WorldCommand) -> Vec<ServerPacket> {
    let payload = server.handle_envelope(host_envelope(ZoneRpcRequest::Execute {
        owner_lease: lease.into(), mode: WireZoneOwnerCommandMode::Direct,
        command: WireWorldCommand::from_world(command).unwrap(),
    })).unwrap();
    let frames = match payload {
        ZoneRpcPayload::Execution { frames, .. } | ZoneRpcPayload::Packets { frames } => frames,
        other => panic!("unexpected real command payload: {other:?}"),
    };
    decode_server_frames(frames).unwrap()
}

fn host_action(server: &ZoneHostServer, lease: &ZoneOwnerLease, authenticated: bool,
    version: u16, action: NpcPurchaseOwnerAction) -> Result<NpcPurchaseOwnerRouteExecution, NpcPurchaseDurableError> {
    let payload = server.handle_envelope(host_envelope(ZoneRpcRequest::NpcPurchaseOwner {
        request: WireNpcPurchaseOwnerRequest { protocol_version: version, owner_lease: lease.into(), authenticated, action },
    })).unwrap();
    let ZoneRpcPayload::NpcPurchaseOwner { response } = payload else { panic!("dedicated owner response required") };
    response.into_execution(action)
}

#[test]
fn durable_library_host_dispatch_fences_auth_version_lease_and_records_only_replay_checkpoint() {
    let (config, path) = owned_config("host-dispatch");
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let lease = authority.owner_lease(&ZoneId::primary());
    // Preinsert synchronous Zone resources with disconnected movement ingress.
    // No owner thread, autonomous tick, listener or server loop is constructed.
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&ZoneId::primary()));
    let server = ZoneHostServer::with_identity_and_factory("source28-test-host", 8, config.clone(),
        authority.clone(), Some(HOST_TOKEN.into()), ZoneRpcLimits::default(), factory);
    let seeded = fs::read(&path).unwrap();
    let mut wrong_version = host_envelope(ZoneRpcRequest::Health);
    wrong_version.protocol_version = ZONE_RPC_PROTOCOL_VERSION + 1;
    assert_eq!(server.handle_envelope(wrong_version).unwrap_err().code, "unsupported_version");
    let mut wrong_token = host_envelope(ZoneRpcRequest::Health);
    wrong_token.auth_token = Some("different-source28-token".into());
    assert_eq!(server.handle_envelope(wrong_token).unwrap_err().code, "unauthorized");
    assert!(server.sessions.lock().unwrap().is_empty());
    assert!(server.journal.lock().unwrap().entries.is_empty());
    assert_eq!(fs::read(&path).unwrap(), seeded);
    let head = server.handle_envelope(host_envelope(ZoneRpcRequest::ReplicationHead)).unwrap();
    let ZoneRpcPayload::ReplicationHead { head } = head else { panic!("actual host replication head") };
    assert!(head.build_id.starts_with("mir2-zone/npc-checkpoint-v1/"));
    assert_eq!(head.build_id, zone_replication_build_id());
    assert_ne!(head.build_id, format!("mir2-gateway/{}", env!("CARGO_PKG_VERSION")));
    let health = server.handle_envelope(host_envelope(ZoneRpcRequest::Health)).unwrap();
    let ZoneRpcPayload::Health { capabilities, .. } = health else { panic!("actual host health") };
    assert!(capabilities.iter().any(|capability| capability == ZONE_RPC_DURABLE_NPC_PURCHASE_OWNER_V1));
    let login = host_command(&server, &lease, WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    }));
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    host_command(&server, &lease, WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }));
    host_command(&server, &lease, WorldCommand::Interact { object_id: NPC });
    let goods = host_command(&server, &lease, WorldCommand::SelectNpcDialog { target: "@BuySell".into() });
    let item_index = goods.iter().find_map(|packet| match packet {
        ServerPacket::NPCGoods { list, .. } => list.iter().find(|item| item.item_index == 658 && item.count == 1)
            .map(|item| item.unique_id), _ => None,
    }).expect("actual hosted NPC selector");
    let request = NpcPurchaseRequest { item_index, count: 2, panel_type: 0 };
    server.handle_envelope(host_envelope(ZoneRpcRequest::SaveActiveCharacter)).unwrap();
    let bytes = fs::read(&path).unwrap();
    let journal_len = server.journal.lock().unwrap().entries.len();
    let host_session = server.sessions.lock().unwrap().get(&(HOST_SESSION.into(), ZoneId::primary().as_str().into())).unwrap().clone();
    let snapshot_before = snapshot_value(&host_session.hosted.world_snapshot().unwrap());
    assert_preflight(host_action(&server, &lease, false, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, NpcPurchaseOwnerAction::Begin));
    assert_preflight(host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION + 1, NpcPurchaseOwnerAction::Begin));
    let stale = ZoneOwnerLease::new(ZoneId::primary(), "source28-not-current-owner", lease.fencing_token());
    assert_preflight(host_action(&server, &stale, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, NpcPurchaseOwnerAction::Begin));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(server.journal.lock().unwrap().entries.len(), journal_len);
    assert_eq!(snapshot_value(&host_session.hosted.world_snapshot().unwrap()), snapshot_before);
    let began = host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, NpcPurchaseOwnerAction::Begin).unwrap();
    let NpcPurchaseOwnerReply::Producer { producer } = began.reply else { panic!("real host producer") };
    assert_eq!(began.authority, Some(producer));
    assert!(began.snapshot.is_some());
    let quoted = host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, NpcPurchaseOwnerAction::Quote { request }).unwrap();
    let NpcPurchaseOwnerReply::Quote { intent } = quoted.reply else { panic!("real hosted intent") };
    let operation = NpcPurchaseOperation { actor: producer.actor, request_scope: producer.producer_scope, sequence: 1, intent };
    let baseline_bytes = fs::read(&path).unwrap();
    let before_purchase_journal = server.journal.lock().unwrap().entries.len();
    let missing = host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, NpcPurchaseOwnerAction::Query { operation }).unwrap();
    assert_eq!(missing.reply, NpcPurchaseOwnerReply::Recovery { receipt: None });
    assert!(missing.execution.packets.is_empty());
    assert_eq!(fs::read(&path).unwrap(), baseline_bytes);
    assert_eq!(server.journal.lock().unwrap().entries.len(), before_purchase_journal);
    // The standby is isolated before the real source mutation, so the following
    // replay can only obtain this commit through its recorded checkpoint arm.
    let replica_config = config.fork_for_replica_apply().unwrap();
    assert!(replica_config.account_store_path.is_none());
    assert!(replica_config.account_store_database_url.is_none());
    let mut replay_runtime: ZoneRuntimeHandle = Box::new(InProcessWorldRuntime::new(replica_config.clone()));
    let replica_login = replay_runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    })).unwrap();
    assert!(replica_login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    replay_runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 })).unwrap();
    let replay_baseline = replay_runtime.active_character_checkpoint().unwrap();
    assert_eq!(replay_baseline.revision, producer.server_revision);
    assert!(replay_baseline.npc_purchase_journal.as_ref().unwrap().entries.is_empty());
    let standby = ZoneHostSession::new(Arc::new(HostedZoneOwnerCommandClient::new(replay_runtime)),
        ZoneRpcLimits::default().max_outbound_messages);
    let action = NpcPurchaseOwnerAction::Purchase { operation };
    let purchased = host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, action).unwrap();
    let receipt = actual_commit(&purchased, operation);
    let committed_bytes = fs::read(&path).unwrap();
    let actual_store: AccountStore = serde_json::from_slice(&committed_bytes).unwrap();
    let actual_checkpoint = actual_store.accounts["demo"].saves[&0].clone();
    assert_eq!(actual_checkpoint.revision, receipt.entry.server_revision);
    assert_eq!(actual_checkpoint.npc_purchase_journal.as_ref().unwrap().entries, vec![receipt.entry.clone()]);
    let entry = server.journal.lock().unwrap().entries.last().unwrap().clone();
    let Some(WireWorldCommand::NpcPurchaseOwnerCheckpoint { checkpoint }) = entry.command.as_ref() else {
        panic!("economic host journal must record an already committed checkpoint");
    };
    assert_eq!(serde_json::to_value(checkpoint).unwrap(), serde_json::to_value(&actual_checkpoint).unwrap());
    assert!(entry.clone().into_request().is_err(), "checkpoint must never become a generic executable BuyItem");
    entry.replay_session_mutation(&standby, entry.sequence, true).unwrap();
    let replayed = standby.hosted.active_character_checkpoint().unwrap().unwrap();
    assert_eq!(replayed.gold, actual_checkpoint.gold);
    assert_eq!(replayed.inventory_items_json, actual_checkpoint.inventory_items_json);
    assert_eq!(replayed.revision, actual_checkpoint.revision);
    assert_eq!(replayed.npc_purchase_journal, actual_checkpoint.npc_purchase_journal);
    assert!(!standby.hosted.supports_durable_npc_purchase_owner());
    let mirrored = replica_config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(serde_json::to_value(&mirrored).unwrap(), serde_json::to_value(&actual_checkpoint).unwrap());
    let memory = serde_json::to_value(&*replica_config.account_store.lock().unwrap()).unwrap();
    entry.replay_session_mutation(&standby, entry.sequence, true).unwrap();
    assert_eq!(serde_json::to_value(&*replica_config.account_store.lock().unwrap()).unwrap(), memory);
    assert_eq!(fs::read(&path).unwrap(), committed_bytes);
    let duplicate = host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, action).unwrap();
    assert_eq!(duplicate.reply, NpcPurchaseOwnerReply::Purchase { receipt: receipt.clone(), replayed: true });
    assert!(duplicate.execution.packets.is_empty());
    let recovered = host_action(&server, &lease, true, NPC_PURCHASE_OWNER_PROTOCOL_VERSION, NpcPurchaseOwnerAction::Query { operation }).unwrap();
    assert_eq!(recovered.reply, NpcPurchaseOwnerReply::Recovery { receipt: Some(receipt) });
    assert!(recovered.execution.packets.is_empty());
    assert_eq!(fs::read(&path).unwrap(), committed_bytes);
}
