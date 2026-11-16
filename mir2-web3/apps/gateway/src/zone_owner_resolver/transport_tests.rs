use super::super::{HostRegistry, ZoneLeaseObserver};
use super::*;
use crate::routing::ZoneId;
use crate::zone_lease::ObservedZoneLease;
use mir2_simulation::{
    GameShopPurchaseOutcome, NativeAttackAdmission, NativeGameShopPurchaseRequest, WorldCommand,
    WorldCommandKind, WorldCommandOutcome,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

const SESSION: &str = "stable-save-session";

// Scripted boundary receipts: these tests do not assert a real database commit.
fn save_receipt(status: &str, binding: &ZoneSaveOperationBinding) -> ZoneSaveOperationReceipt {
    let config = crate::GatewayConfig::default();
    let index = config.default_character.index;
    let frozen = config.account_store.lock().unwrap().accounts["demo"].saves[&index].clone();
    let mut expected = frozen.clone();
    expected.revision += 1;
    let frozen = serde_json::to_value(frozen).unwrap();
    let expected = serde_json::to_value(expected).unwrap();
    let digest = |value: &serde_json::Value| {
        Sha256::digest(serde_json::to_vec(value).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    ZoneSaveOperationReceipt::from_json_checked(serde_json::json!({
        "operation": { "binding": binding.to_json().unwrap(), "accountId": "demo",
            "frozenCheckpoint": frozen, "expectedSavedCheckpoint": expected,
            "checkpointDigest": digest(&frozen), "expectedDigest": digest(&expected),
            "lastAccessBinaryDatetime": null },
        "status": status,
        "savedCheckpoint": if status == "Prepared" { None } else { Some(expected) },
        "accountStoreVersion": if status == "Prepared" { None } else { Some(4) },
        "characterSaveVersion": if status == "Prepared" { None } else { Some(3) }
    }))
    .unwrap()
}

#[derive(Debug)]
struct Observer {
    row: Mutex<Option<ObservedZoneLease>>,
    fail: AtomicBool,
}

impl Observer {
    fn put(&self, owner: &str, epoch: u64, expiry: u64) {
        *self.row.lock().unwrap() = Some(ObservedZoneLease {
            owner_id: owner.into(),
            fencing_token: epoch,
            expires_at_ms: expiry,
            heartbeat_at_ms: 100,
        });
    }
}

impl ZoneLeaseObserver for Observer {
    fn observe(&self, _: &ZoneId) -> Result<Option<ObservedZoneLease>, ResolveError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(ResolveError::ObserverUnavailable);
        }
        Ok(self.row.lock().unwrap().clone())
    }
}

#[derive(Debug)]
struct FakeTransport {
    observer: Arc<Observer>,
    calls: Mutex<Vec<&'static str>>,
    mutation_calls: AtomicUsize,
    probe_calls: AtomicUsize,
    after_mutation: Mutex<Option<(&'static str, u64)>>,
    mutation_error: Mutex<Option<String>>,
    flip_during_read: AtomicBool,
    flip_during_capability: AtomicBool,
    save_capability: AtomicBool,
    save_empty: AtomicBool,
    save_publish_on_error: AtomicBool,
    save_receipt: Mutex<Option<ZoneSaveOperationReceipt>>,
    save_bindings: Mutex<Vec<ZoneSaveOperationBinding>>,
}

impl FakeTransport {
    fn record(&self, name: &'static str) {
        self.calls.lock().unwrap().push(name);
    }

    fn mutate(&self, name: &'static str) -> Result<(), String> {
        self.record(name);
        self.mutation_calls.fetch_add(1, Ordering::SeqCst);
        if let Some((owner, epoch)) = self.after_mutation.lock().unwrap().take() {
            self.observer.put(owner, epoch, 500);
        }
        match self.mutation_error.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn capability(&self, name: &'static str) -> bool {
        self.record(name);
        if self.flip_during_capability.swap(false, Ordering::SeqCst) {
            self.observer.put("host-b", 8, 500);
        }
        true
    }
}

fn execution() -> WorldCommandExecution {
    WorldCommandExecution {
        packets: vec![],
        outcome: WorldCommandOutcome {
            command_kind: WorldCommandKind::Attack,
            packet_count: 0,
            snapshot_tick: 42,
            active_identity: None,
        },
        game_shop_purchase_outcome: None,
    }
}

impl ZoneOwnerRpcTransport for FakeTransport {
    fn supports_zone_save_operations(&self) -> bool {
        self.capability("save_capability") && self.save_capability.load(Ordering::SeqCst)
    }

    fn prepare_zone_save_operation(
        &self,
        lease: &ZoneOwnerLease,
        operation_id: &str,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        let binding = ZoneSaveOperationBinding::new(
            operation_id.into(),
            SESSION.into(),
            ZoneSaveFence::new(
                lease.zone_id().as_str().into(),
                lease.owner_id().into(),
                lease.fencing_token(),
            )
            .unwrap(),
        )
        .unwrap();
        self.save_bindings.lock().unwrap().push(binding.clone());
        let result = self.mutate("save_prepare");
        if !self.save_empty.load(Ordering::SeqCst)
            && (result.is_ok() || self.save_publish_on_error.load(Ordering::SeqCst))
        {
            *self.save_receipt.lock().unwrap() = Some(save_receipt("Prepared", &binding));
        }
        result?;
        self.save_receipt
            .lock()
            .unwrap()
            .as_ref()
            .map(PreparedZoneTeardown::from_remote_operation)
            .transpose()
    }

    fn persist_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        self.save_bindings.lock().unwrap().push(binding.clone());
        let result = self.mutate("save_persist");
        if result.is_ok() || self.save_publish_on_error.load(Ordering::SeqCst) {
            *self.save_receipt.lock().unwrap() = Some(save_receipt("Committed", binding));
        }
        result?;
        Ok(self.save_receipt.lock().unwrap().clone().unwrap())
    }

    fn query_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<Option<ZoneSaveOperationReceipt>, String> {
        self.record("save_query");
        let receipt = self.save_receipt.lock().unwrap().clone();
        if receipt
            .as_ref()
            .is_some_and(|r| r.operation().operation_id() != binding.operation_id())
        {
            return Ok(None);
        }
        if receipt
            .as_ref()
            .is_some_and(|r| r.operation().binding() != binding)
        {
            return Err(SAVE_RECOVERY.into());
        }
        Ok(receipt)
    }

    fn release_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        self.save_bindings.lock().unwrap().push(binding.clone());
        let result = self.mutate("save_release");
        if result.is_ok() || self.save_publish_on_error.load(Ordering::SeqCst) {
            *self.save_receipt.lock().unwrap() = Some(save_receipt("Released", binding));
        }
        result?;
        Ok(self.save_receipt.lock().unwrap().clone().unwrap())
    }

    fn execute_production_attack_requiring_typed_outcome(
        &self,
        _: ZoneOwnerCommandRequest,
    ) -> Result<NativeAttackExecution, NativeAttackError> {
        self.mutate("native_attack")
            .map_err(NativeAttackError::Unknown)?;
        Ok(NativeAttackExecution {
            execution: execution(),
            admission: NativeAttackAdmission::Launched,
        })
    }

    fn supports_durable_npc_purchase_owner(&self) -> bool {
        self.capability("npc_capability")
    }

    fn execute_npc_purchase_owner(
        &self,
        _: &ZoneOwnerLease,
        _: bool,
        _: NpcPurchaseOwnerAction,
    ) -> Result<NpcPurchaseOwnerRouteExecution, NpcPurchaseDurableError> {
        self.mutate("npc_purchase")
            .map_err(|detail| NpcPurchaseDurableError::Unknown { detail })?;
        Err(NpcPurchaseDurableError::Unknown {
            detail: "npc owner response lost".into(),
        })
    }

    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        self.mutate("on_connect")?;
        Ok(vec![])
    }

    fn execute(&self, _: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.mutate("execute")?;
        Ok(execution())
    }

    fn execute_requiring_typed_game_shop_purchase_outcome(
        &self,
        _: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.mutate("typed_shop")?;
        Ok(shop_execution())
    }

    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        self.capability("shop_capability")
    }

    fn supports_native_game_shop_gifts(&self) -> bool {
        self.capability("gift_capability")
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        self.record("snapshot");
        Err("snapshot sentinel".into())
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        self.record("identity");
        if self.flip_during_read.swap(false, Ordering::SeqCst) {
            self.observer.put("host-b", 8, 500);
        }
        Ok(None)
    }

    fn active_character_checkpoint(&self) -> Result<Option<CharacterSaveRecord>, String> {
        self.record("checkpoint");
        Ok(None)
    }

    fn prepare_character_handoff(
        &self,
        _: &ZoneOwnerLease,
    ) -> Result<CharacterHandoffCheckpoint, String> {
        self.mutate("handoff")?;
        Err("handoff sentinel".into())
    }

    fn prepare_teardown_checkpoint(
        &self,
        _: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        self.mutate("teardown")?;
        Ok(None)
    }

    fn release_teardown_fence(&self) -> Result<(), String> {
        self.mutate("release")
    }

    fn save_active_character(&self) -> Result<(), String> {
        self.mutate("save")
    }

    fn refresh_active_external_mail(&self) -> Result<bool, String> {
        self.mutate("mail")?;
        Ok(true)
    }

    fn close_session(&self, _: &ZoneOwnerLease) -> Result<(), String> {
        self.mutate("close")
    }

    fn register_live_outbound(
        &self,
        _: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        self.record("live_outbound");
        Ok(None)
    }
}

struct Fixture {
    resolver: Arc<StableZoneOwnerResolver>,
    observer: Arc<Observer>,
    inner: Arc<FakeTransport>,
    now: Arc<AtomicU64>,
    lease: ZoneOwnerLease,
}

impl Fixture {
    fn new() -> Self {
        let observer = Arc::new(Observer {
            row: Mutex::new(None),
            fail: AtomicBool::new(false),
        });
        observer.put("host-a", 7, 200);
        let now = Arc::new(AtomicU64::new(100));
        let clock = now.clone();
        let registry = HostRegistry::from_json(
            r#"{"version":1,"hosts":[{"ownerId":"host-a","endpoint":"127.0.0.1:7401"},{"ownerId":"host-b","endpoint":"127.0.0.1:7402"}]}"#,
        ).unwrap();
        Self {
            resolver: Arc::new(StableZoneOwnerResolver::new(
                registry,
                observer.clone(),
                Arc::new(move || clock.load(Ordering::SeqCst)),
            )),
            inner: Arc::new(FakeTransport {
                observer: observer.clone(),
                calls: Mutex::new(vec![]),
                mutation_calls: AtomicUsize::new(0),
                probe_calls: AtomicUsize::new(0),
                after_mutation: Mutex::new(None),
                mutation_error: Mutex::new(None),
                flip_during_read: AtomicBool::new(false),
                flip_during_capability: AtomicBool::new(false),
                save_capability: AtomicBool::new(false),
                save_empty: AtomicBool::new(true),
                save_publish_on_error: AtomicBool::new(false),
                save_receipt: Mutex::new(None),
                save_bindings: Mutex::new(vec![]),
            }),
            observer,
            now,
            lease: ZoneOwnerLease::new(ZoneId::new("map:0"), "host-a", 7),
        }
    }

    fn transport(&self, host_id: &str) -> Result<StableZoneOwnerRpcTransport, String> {
        let pin = self
            .resolver
            .require_lease(&self.lease)
            .map_err(|e| e.to_string())?;
        StableZoneOwnerRpcTransport::from_transport(
            self.resolver.clone(),
            pin,
            SESSION.into(),
            self.inner.clone(),
            || {
                self.inner.probe_calls.fetch_add(1, Ordering::SeqCst);
                Ok(host_id.to_string())
            },
        )
    }

    fn request(&self) -> ZoneOwnerCommandRequest {
        ZoneOwnerCommandRequest::production_player(
            self.lease.clone(),
            true,
            WorldCommand::Attack { object_id: 123 },
        )
    }
}

#[test]
fn a_wrong_real_host_identity_cannot_create_a_bound_transport() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.transport("host-b").unwrap_err(),
        "zone_owner_host_identity_mismatch"
    );
    assert_eq!(fixture.inner.probe_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
    assert!(fixture.inner.calls.lock().unwrap().is_empty());
}

#[test]
fn identity_probe_requires_exact_owner_spelling() {
    for host_id in ["HOST-A", "host-a ", "host-a:alias", ""] {
        let fixture = Fixture::new();
        assert!(fixture.transport(host_id).is_err());
        assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn an_epoch_change_during_the_initial_health_probe_rejects_the_binding() {
    let fixture = Fixture::new();
    let pin = fixture.resolver.require_lease(&fixture.lease).unwrap();
    let result = StableZoneOwnerRpcTransport::from_transport(
        fixture.resolver.clone(),
        pin,
        SESSION.into(),
        fixture.inner.clone(),
        || {
            fixture.observer.put("host-a", 8, 500);
            Ok("host-a".into())
        },
    );
    assert_eq!(result.unwrap_err(), ResolveError::RequiresRebind.code());
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn same_epoch_renewal_reuses_one_transport_and_one_identity_probe() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    assert_eq!(transport.execute(fixture.request()).unwrap(), execution());
    fixture.now.store(250, Ordering::SeqCst);
    fixture.observer.put("host-a", 7, 500);
    assert_eq!(transport.execute(fixture.request()).unwrap(), execution());
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.inner.probe_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        *fixture.inner.calls.lock().unwrap(),
        vec!["execute", "execute"]
    );
}

#[test]
fn owner_or_epoch_change_dispatches_no_old_owner_mutations_or_reads() {
    for (owner, epoch) in [("host-a", 8), ("host-b", 8), ("unknown-host", 8)] {
        let fixture = Fixture::new();
        let transport = fixture.transport("host-a").unwrap();
        fixture.observer.put(owner, epoch, 500);
        let expected = ResolveError::RequiresRebind.code();
        assert_eq!(transport.execute(fixture.request()).unwrap_err(), expected);
        assert_eq!(
            transport
                .execute_requiring_typed_game_shop_purchase_outcome(fixture.request())
                .unwrap_err(),
            expected
        );
        assert!(
            matches!(transport.execute_production_attack_requiring_typed_outcome(fixture.request()), Err(NativeAttackError::BeforeExecution(detail)) if detail == expected)
        );
        assert!(
            matches!(transport.execute_npc_purchase_owner(&fixture.lease, true, NpcPurchaseOwnerAction::Begin), Err(NpcPurchaseDurableError::BeforeExecution { detail }) if detail == expected)
        );
        assert_eq!(transport.on_connect().unwrap_err(), expected);
        assert_eq!(transport.save_active_character().unwrap_err(), expected);
        assert_eq!(
            transport.refresh_active_external_mail().unwrap_err(),
            expected
        );
        assert_eq!(
            transport
                .prepare_character_handoff(&fixture.lease)
                .unwrap_err(),
            expected
        );
        assert_eq!(
            transport
                .prepare_teardown_checkpoint(&fixture.lease)
                .unwrap_err(),
            expected
        );
        assert_eq!(transport.release_teardown_fence().unwrap_err(), expected);
        assert_eq!(
            transport.close_session(&fixture.lease).unwrap_err(),
            expected
        );
        assert_eq!(transport.world_snapshot().unwrap_err(), expected);
        assert_eq!(transport.active_identity().unwrap_err(), expected);
        assert_eq!(
            transport.active_character_checkpoint().unwrap_err(),
            expected
        );
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        assert_eq!(
            transport
                .register_live_outbound(SharedZoneLiveOutboundSender::single(sender))
                .unwrap_err(),
            expected
        );
        assert!(!transport.supports_typed_game_shop_purchase_outcome());
        assert!(!transport.supports_native_game_shop_gifts());
        assert!(!transport.supports_durable_npc_purchase_owner());
        assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
        assert!(fixture.inner.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn request_lease_cannot_switch_the_transport_to_a_different_owner_epoch_or_zone() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    for lease in [
        ZoneOwnerLease::new(ZoneId::new("map:0"), "host-a", 8),
        ZoneOwnerLease::new(ZoneId::new("map:0"), "host-b", 7),
        ZoneOwnerLease::new(ZoneId::new("map:1"), "host-a", 7),
    ] {
        let request = ZoneOwnerCommandRequest::production_player(
            lease.clone(),
            true,
            WorldCommand::Attack { object_id: 123 },
        );
        assert_eq!(
            transport.execute(request).unwrap_err(),
            ResolveError::RequiresRebind.code()
        );
        assert_eq!(
            transport.close_session(&lease).unwrap_err(),
            ResolveError::RequiresRebind.code()
        );
    }
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn expired_removed_or_unavailable_observation_never_reuses_previous_authorization() {
    for fault in 0..3 {
        let fixture = Fixture::new();
        let transport = fixture.transport("host-a").unwrap();
        let expected = match fault {
            0 => {
                fixture.now.store(200, Ordering::SeqCst);
                ResolveError::LeaseExpired
            }
            1 => {
                *fixture.observer.row.lock().unwrap() = None;
                ResolveError::LeaseNotRegistered
            }
            _ => {
                fixture.observer.fail.store(true, Ordering::SeqCst);
                ResolveError::ObserverUnavailable
            }
        };
        assert_eq!(
            transport.execute(fixture.request()).unwrap_err(),
            expected.code()
        );
        assert!(!transport.supports_native_game_shop_gifts());
        assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn capabilities_and_specialized_native_methods_are_forwarded() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    assert!(transport.supports_typed_game_shop_purchase_outcome());
    assert!(transport.supports_native_game_shop_gifts());
    assert!(transport.supports_durable_npc_purchase_owner());
    assert_eq!(
        transport
            .execute_requiring_typed_game_shop_purchase_outcome(fixture.request())
            .unwrap(),
        shop_execution()
    );
    assert_eq!(
        transport
            .execute_production_attack_requiring_typed_outcome(fixture.request())
            .unwrap()
            .admission,
        NativeAttackAdmission::Launched
    );
    assert!(
        matches!(transport.execute_npc_purchase_owner(&fixture.lease, true, NpcPurchaseOwnerAction::Begin), Err(NpcPurchaseDurableError::Unknown { detail }) if detail == "npc owner response lost")
    );
    assert_eq!(
        *fixture.inner.calls.lock().unwrap(),
        vec![
            "shop_capability",
            "gift_capability",
            "npc_capability",
            "typed_shop",
            "native_attack",
            "npc_purchase"
        ]
    );
}

#[test]
fn lifecycle_and_read_methods_keep_the_inner_behavior() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    assert!(transport.on_connect().unwrap().is_empty());
    assert_eq!(transport.world_snapshot().unwrap_err(), "snapshot sentinel");
    assert_eq!(transport.active_identity().unwrap(), None);
    assert!(transport.active_character_checkpoint().unwrap().is_none());
    assert_eq!(
        transport
            .prepare_character_handoff(&fixture.lease)
            .unwrap_err(),
        "handoff sentinel"
    );
    assert!(transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap()
        .is_none());
    transport.release_teardown_fence().unwrap();
    transport.save_active_character().unwrap();
    assert!(transport.refresh_active_external_mail().unwrap());
    transport.close_session(&fixture.lease).unwrap();
    let (sender, _receiver) = tokio::sync::mpsc::channel(1);
    assert!(transport
        .register_live_outbound(SharedZoneLiveOutboundSender::single(sender))
        .unwrap()
        .is_none());
    assert_eq!(
        *fixture.inner.calls.lock().unwrap(),
        vec![
            "on_connect",
            "snapshot",
            "identity",
            "checkpoint",
            "handoff",
            "save_capability",
            "save_capability",
            "save_query",
            "save_capability",
            "save_prepare",
            "save_capability",
            "save_capability",
            "save_capability",
            "save_query",
            "save_capability",
            "save_prepare",
            "mail",
            "close",
            "live_outbound"
        ]
    );
}

#[test]
fn a_confirmed_mutation_survives_an_epoch_change_during_the_call() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    *fixture.inner.after_mutation.lock().unwrap() = Some(("host-b", 8));
    assert_eq!(transport.execute(fixture.request()).unwrap(), execution());
    assert_eq!(
        transport.execute(fixture.request()).unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn an_unknown_mutation_outcome_is_preserved_and_never_replayed() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    *fixture.inner.after_mutation.lock().unwrap() = Some(("host-b", 8));
    *fixture.inner.mutation_error.lock().unwrap() = Some("commit state is unknown".into());
    assert_eq!(
        transport.execute(fixture.request()).unwrap_err(),
        "commit state is unknown"
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 1);
    assert_eq!(*fixture.inner.calls.lock().unwrap(), vec!["execute"]);
}

#[test]
fn native_unknown_keeps_its_typed_classification_after_epoch_change() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    *fixture.inner.after_mutation.lock().unwrap() = Some(("host-b", 8));
    *fixture.inner.mutation_error.lock().unwrap() = Some("attack reply lost".into());
    assert_eq!(
        transport
            .execute_production_attack_requiring_typed_outcome(fixture.request())
            .unwrap_err(),
        NativeAttackError::Unknown("attack reply lost".into())
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_read_that_crosses_an_owner_change_does_not_publish_its_old_result() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    fixture.inner.flip_during_read.store(true, Ordering::SeqCst);
    assert_eq!(
        transport.active_identity().unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
    assert_eq!(*fixture.inner.calls.lock().unwrap(), vec!["identity"]);
}

#[test]
fn a_capability_probe_that_crosses_an_owner_change_fails_closed() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    fixture
        .inner
        .flip_during_capability
        .store(true, Ordering::SeqCst);
    assert!(!transport.supports_typed_game_shop_purchase_outcome());
    assert_eq!(
        *fixture.inner.calls.lock().unwrap(),
        vec!["shop_capability"]
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_different_endpoint_cannot_reuse_an_existing_pin_or_probe_the_new_host() {
    let fixture = Fixture::new();
    let pin = fixture.resolver.require_lease(&fixture.lease).unwrap();
    let changed_registry = HostRegistry::from_json(
        r#"{"version":1,"hosts":[{"ownerId":"host-a","endpoint":"127.0.0.1:7499"}]}"#,
    )
    .unwrap();
    let clock = fixture.now.clone();
    let changed_resolver = Arc::new(StableZoneOwnerResolver::new(
        changed_registry,
        fixture.observer.clone(),
        Arc::new(move || clock.load(Ordering::SeqCst)),
    ));
    let result = StableZoneOwnerRpcTransport::from_transport(
        changed_resolver,
        pin,
        SESSION.into(),
        fixture.inner.clone(),
        || panic!("an endpoint change must be rejected before health or mutation"),
    );
    assert_eq!(result.unwrap_err(), ResolveError::RequiresRebind.code());
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
}

fn shop_execution() -> WorldCommandExecution {
    let mut result = execution();
    result.game_shop_purchase_outcome = Some(GameShopPurchaseOutcome {
        success: true,
        g_index: 17,
        quantity: 1,
        price_type: 2,
        new_stock_level: Some(9),
        mail_id: Some(12345),
        failure: None,
    });
    result
}

#[test]
fn confirmed_typed_economic_success_is_not_relabelled_when_the_epoch_changes() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    *fixture.inner.after_mutation.lock().unwrap() = Some(("host-b", 8));
    let request = ZoneOwnerCommandRequest::production_player(
        fixture.lease.clone(),
        true,
        WorldCommand::NativeGameShopPurchase(NativeGameShopPurchaseRequest {
            protocol_version: 1,
            server_idempotency_key: "server-key".into(),
            gateway_session_id: "gateway-session".into(),
            account_id: "account-a".into(),
            character_index: 0,
            client_request_id: "client-request".into(),
            g_index: 17,
            quantity: 1,
            price_type: 2,
        }),
    );
    assert_eq!(
        transport
            .execute_requiring_typed_game_shop_purchase_outcome(request)
            .unwrap(),
        shop_execution()
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        transport.save_active_character().unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn legacy_save_lifecycle_requires_the_new_capability_without_any_fallback() {
    let fixture = Fixture::new();
    let transport = fixture.transport("host-a").unwrap();
    assert!(!transport.supports_zone_save_operations());
    assert_eq!(
        transport
            .prepare_teardown_checkpoint(&fixture.lease)
            .unwrap_err(),
        "zone_save_operations_unsupported"
    );
    assert_eq!(
        transport.save_active_character().unwrap_err(),
        "zone_save_operations_unsupported"
    );
    assert_eq!(
        transport.release_teardown_fence().unwrap_err(),
        "zone_save_operations_unsupported"
    );
    assert_eq!(fixture.inner.mutation_calls.load(Ordering::SeqCst), 0);
    assert!(transport.pending_save.lock().unwrap().is_none());
    assert!(fixture
        .inner
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|call| *call == "save_capability"));
}

#[test]
fn legacy_save_completes_only_through_one_immutable_receipt_binding() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    fixture.inner.save_empty.store(false, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    transport.save_active_character().unwrap();
    assert!(transport.pending_save.lock().unwrap().is_none());
    let first = fixture.inner.save_bindings.lock().unwrap().clone();
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|binding| binding == &first[0]));
    assert_eq!(first[0].rpc_session_id(), SESSION);
    assert_eq!(first[0].fence().fencing_token(), 7);
    assert_eq!(
        fixture
            .inner
            .save_receipt
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Released
    );
    transport.save_active_character().unwrap();
    let all = fixture.inner.save_bindings.lock().unwrap();
    assert_eq!(all.len(), 6);
    assert_ne!(all[0].operation_id(), all[3].operation_id());
    assert!(all[3..].iter().all(|binding| binding == &all[3]));
    assert!(fixture
        .inner
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|call| !matches!(*call, "save" | "teardown" | "release")));
}

#[test]
fn lost_prepare_reply_keeps_the_same_id_and_requires_readonly_reconciliation() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    fixture.inner.save_empty.store(false, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    *fixture.inner.mutation_error.lock().unwrap() =
        Some("ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN: prepare reply lost".into());
    assert!(transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    let original = transport
        .pending_save
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .binding
        .clone();
    *fixture.inner.mutation_error.lock().unwrap() = None;
    assert!(transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert_eq!(fixture.inner.save_bindings.lock().unwrap().len(), 1);
    assert_eq!(
        transport
            .prepare_zone_save_operation(&fixture.lease, "different-operation")
            .unwrap_err(),
        "zone_save_operation_pending"
    );
    assert_eq!(
        transport
            .pending_save
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .binding,
        original
    );
    *fixture.inner.save_receipt.lock().unwrap() = Some(save_receipt("Prepared", &original));
    let prepared = transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.remote_operation_binding(), Some(&original));
    transport.persist_teardown_checkpoint(&prepared).unwrap();
    assert_eq!(fixture.inner.save_bindings.lock().unwrap().len(), 2);
    assert!(fixture
        .inner
        .save_bindings
        .lock()
        .unwrap()
        .iter()
        .all(|b| b == &original));
}

#[test]
fn committed_old_epoch_receipt_resolves_unknown_without_replay_or_rebinding() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    fixture.inner.save_empty.store(false, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    let prepared = transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap()
        .unwrap();
    let binding = prepared.remote_operation_binding().unwrap().clone();
    fixture
        .inner
        .save_publish_on_error
        .store(true, Ordering::SeqCst);
    *fixture.inner.mutation_error.lock().unwrap() =
        Some("ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN: persist reply lost".into());
    *fixture.inner.after_mutation.lock().unwrap() = Some(("host-b", 8));
    assert!(transport
        .persist_teardown_checkpoint(&prepared)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    let mutations = fixture.inner.mutation_calls.load(Ordering::SeqCst);
    // The receipt query precedes observer/lease admission, including an outage.
    fixture.observer.fail.store(true, Ordering::SeqCst);
    transport.persist_teardown_checkpoint(&prepared).unwrap();
    assert_eq!(
        transport
            .query_zone_save_operation(&binding)
            .unwrap()
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Committed
    );
    assert_eq!(
        transport
            .prepare_teardown_checkpoint(&fixture.lease)
            .unwrap()
            .unwrap()
            .remote_operation_binding(),
        Some(&binding)
    );
    assert_eq!(
        transport.release_teardown_fence().unwrap_err(),
        ResolveError::ObserverUnavailable.code()
    );
    assert_eq!(
        fixture.inner.mutation_calls.load(Ordering::SeqCst),
        mutations
    );
    assert_eq!(transport.pin.lease(), &fixture.lease);
    assert_eq!(
        transport
            .pending_save
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .binding,
        binding
    );
}

#[test]
fn lost_persist_reply_with_only_prepared_receipt_never_reissues_a_mutation() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    fixture.inner.save_empty.store(false, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    let prepared = transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap()
        .unwrap();
    *fixture.inner.mutation_error.lock().unwrap() =
        Some("ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN: persist response absent".into());
    assert!(transport
        .persist_teardown_checkpoint(&prepared)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    *fixture.inner.mutation_error.lock().unwrap() = None;
    let mutations = fixture.inner.mutation_calls.load(Ordering::SeqCst);
    assert!(transport
        .persist_teardown_checkpoint(&prepared)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert!(transport
        .save_active_character()
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert_eq!(
        fixture.inner.mutation_calls.load(Ordering::SeqCst),
        mutations
    );
    assert!(transport.pending_save.lock().unwrap().is_some());
}

#[test]
fn released_receipt_requires_live_host_release_confirmation_before_clearing_pending() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    fixture.inner.save_empty.store(false, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    let prepared = transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap()
        .unwrap();
    let binding = prepared.remote_operation_binding().unwrap().clone();
    transport.persist_teardown_checkpoint(&prepared).unwrap();
    fixture
        .inner
        .save_publish_on_error
        .store(true, Ordering::SeqCst);
    *fixture.inner.mutation_error.lock().unwrap() =
        Some("ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN: release reply lost".into());
    *fixture.inner.after_mutation.lock().unwrap() = Some(("host-b", 8));
    assert!(transport
        .release_teardown_fence()
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert!(transport.pending_save.lock().unwrap().is_some());
    let mutations = fixture.inner.mutation_calls.load(Ordering::SeqCst);
    assert_eq!(
        transport.release_teardown_fence().unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
    assert!(transport.pending_save.lock().unwrap().is_some());
    assert_eq!(
        transport
            .query_zone_save_operation(&binding)
            .unwrap()
            .unwrap()
            .status(),
        ZoneSaveOperationStatus::Released
    );
    assert_eq!(
        fixture.inner.mutation_calls.load(Ordering::SeqCst),
        mutations
    );
    // A read-only Released row cannot authorize the old Host while ownership
    // differs. Returning to this same pinned epoch permits explicit same-ID
    // Host confirmation; a frozen Host may still refuse it.
    fixture.observer.put("host-a", 7, 500);
    *fixture.inner.mutation_error.lock().unwrap() =
        Some("zone_save_operation_requires_recovery".into());
    assert_eq!(
        transport.release_teardown_fence().unwrap_err(),
        SAVE_RECOVERY
    );
    assert!(transport.pending_save.lock().unwrap().is_some());
    *fixture.inner.mutation_error.lock().unwrap() = None;
    transport.release_teardown_fence().unwrap();
    assert!(transport.pending_save.lock().unwrap().is_none());
    assert_eq!(
        fixture.inner.mutation_calls.load(Ordering::SeqCst),
        mutations + 2
    );
    assert!(fixture
        .inner
        .save_bindings
        .lock()
        .unwrap()
        .iter()
        .all(|b| b == &binding));
}

#[test]
fn pending_binding_cannot_switch_operation_session_owner_zone_or_epoch() {
    let fixture = Fixture::new();
    fixture.inner.save_capability.store(true, Ordering::SeqCst);
    fixture.inner.save_empty.store(false, Ordering::SeqCst);
    let transport = fixture.transport("host-a").unwrap();
    let prepared = transport
        .prepare_teardown_checkpoint(&fixture.lease)
        .unwrap()
        .unwrap();
    let original = prepared.remote_operation_binding().unwrap();
    let calls = fixture.inner.calls.lock().unwrap().len();
    for binding in [
        ZoneSaveOperationBinding::new(
            "foreign-operation".into(),
            SESSION.into(),
            original.fence().clone(),
        )
        .unwrap(),
        ZoneSaveOperationBinding::new(
            original.operation_id().into(),
            "foreign-session".into(),
            original.fence().clone(),
        )
        .unwrap(),
        ZoneSaveOperationBinding::new(
            original.operation_id().into(),
            SESSION.into(),
            ZoneSaveFence::new("map:1".into(), "host-a".into(), 7).unwrap(),
        )
        .unwrap(),
        ZoneSaveOperationBinding::new(
            original.operation_id().into(),
            SESSION.into(),
            ZoneSaveFence::new("map:0".into(), "host-b".into(), 7).unwrap(),
        )
        .unwrap(),
        ZoneSaveOperationBinding::new(
            original.operation_id().into(),
            SESSION.into(),
            ZoneSaveFence::new("map:0".into(), "host-a".into(), 8).unwrap(),
        )
        .unwrap(),
    ] {
        assert!(transport.persist_zone_save_operation(&binding).is_err());
        assert!(transport.query_zone_save_operation(&binding).is_err());
        assert!(transport.release_zone_save_operation(&binding).is_err());
    }
    assert_eq!(fixture.inner.calls.lock().unwrap().len(), calls);
    assert_eq!(
        transport
            .pending_save
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .binding,
        *original
    );
}
