//! Genuine authenticated loopback RPC, with explicitly prepared initial HP.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;
use crate::InMemoryZoneOwnerLeaseAuthority;
use mir2_protocol::{MirClass, MirDirection, MirGender, MirGridType};

struct Fixture {
    address: SocketAddr,
    server: Arc<ZoneHostServer>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    transport: TcpZoneOwnerRpcTransport,
    lease: ZoneOwnerLease,
    ack: u64,
}

impl Fixture {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let lease = authority.owner_lease(&ZoneId::primary());
        let limits = ZoneRpcLimits::default();
        let server = Arc::new(ZoneHostServer::with_options_and_factory(
            GatewayConfig::default().with_platinum_176_profile(),
            authority,
            Some("health-private-loopback".into()),
            limits.clone(),
            Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
                Duration::from_secs(3600),
                BTreeMap::new(),
            )),
        ));
        let running = server.clone();
        let running_stop = stop.clone();
        let handle = thread::spawn(move || running.serve_until(listener, running_stop).unwrap());
        let transport = TcpZoneOwnerRpcTransport::with_options(
            address.to_string(),
            ZoneId::primary(),
            "health-rpc-session",
            Some("health-private-loopback".into()),
            limits,
        );
        let mut fixture = Self {
            address,
            server,
            stop,
            handle: Some(handle),
            transport,
            lease,
            ack: 0,
        };
        fixture.execute(WorldCommand::ClientPacket(ClientPacket::NewAccount {
            account_id: "healthrpc".into(),
            password: "fixture-password".into(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        }));
        fixture.execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "healthrpc".into(),
            password: "fixture-password".into(),
        }));
        let character = fixture
            .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
                name: "HealthRpc".into(),
                class: MirClass::Warrior,
                gender: MirGender::Male,
            }))
            .packets
            .into_iter()
            .find_map(|p| match p {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .unwrap();
        fixture.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: character,
        }));
        // An explicit durable initial inventory supplies two real native
        // potions for the two sides of the map-transfer test. Each recovery
        // still consumes one ordinary UseItem; no health frame is fabricated.
        let mut initial = fixture
            .transport
            .active_character_checkpoint()
            .unwrap()
            .unwrap();
        for item in &mut initial.inventory_items_json {
            let mut value: serde_json::Value = serde_json::from_str(item).unwrap();
            if value["name"] == "(HP)DrugSmall" {
                value["quantity"] = serde_json::json!(2);
                *item = serde_json::to_string(&value).unwrap();
            }
        }
        fixture
            .transport
            .restore_active_character_checkpoint(&initial)
            .unwrap();
        fixture.clear();
        fixture
    }

    fn execute(&self, command: WorldCommand) -> WorldCommandExecution {
        let stage = match &command {
            WorldCommand::ClientPacket(packet) => format!("{:?}", packet.packet_id()),
            other => format!("{:?}", std::mem::discriminant(other)),
        };
        self.transport
            .execute(ZoneOwnerCommandRequest::direct(self.lease.clone(), command))
            .unwrap_or_else(|error| panic!("private fixture {stage}: {error}"))
    }

    fn clear(&mut self) {
        for _ in 0..16 {
            let batch = self.transport.poll_outbounds(self.ack, 128).unwrap();
            if let Some(last) = batch.items.last() {
                self.ack = last.sequence;
            } else {
                return;
            }
        }
        panic!("bounded fixture bootstrap failed to drain");
    }

    fn drug_health(&mut self) -> SequencedZoneHostPacket {
        let snapshot = self.transport.world_snapshot().unwrap();
        let potion = snapshot
            .inventory_items
            .iter()
            .find(|item| item.name == "(HP)DrugSmall")
            .expect("durable native potion fixture was consumed or not loaded")
            .unique_id;
        let owner = snapshot
            .entities
            .iter()
            .find(|e| e.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
            .unwrap();
        // Only this preparation uses a server handoff; actual recovery below
        // uses ordinary UseItem/Tick, with no fabricated HealthChanged frame.
        self.execute(WorldCommand::ApplyHandoffTransform {
            position: Point {
                x: owner.x,
                y: owner.y,
            },
            direction: MirDirection::Right,
            hp: Some(10),
            mp: None,
        });
        self.clear();
        self.execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: potion,
            grid: MirGridType::Inventory,
        }));
        self.execute(WorldCommand::Tick);
        self.transport
            .poll_outbounds(self.ack, 128)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.owner_health.is_some())
            .expect("actual drug recovery must carry a Host health proof")
    }

    fn validate(&self, item: &SequencedZoneHostPacket) -> bool {
        matches!(
            self.transport
                .call(ZoneRpcRequest::ValidateOwnerHealth {
                    source_registration_id: item.source_registration_id,
                    change: item.owner_health.clone().unwrap(),
                })
                .unwrap(),
            ZoneRpcPayload::Bool { value: true }
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

#[test]
fn owner_health_rpc_validation_is_authenticated_read_only_and_rejects_wrong_source() {
    let mut f = Fixture::new();
    let item = f.drug_health();
    let before_snapshot = f.transport.world_snapshot().unwrap();
    let before_journal = f.server.journal.lock().unwrap().entries.len();
    let before_sessions = f.server.session_count();
    assert!(f.validate(&item));
    let original = item.owner_health.clone().unwrap();
    for index in 0..6 {
        let mut changed = original.clone();
        match index {
            0 => changed.cursor.online_owner = "unissued-node".into(),
            1 => changed.cursor.object_id += 1,
            2 => changed.cursor.life_generation += 1,
            3 => changed.cursor.health_sequence += 1000,
            4 => changed.cursor.session_id = mir2_simulation::SessionId::new("other-session"),
            _ => changed.hp = -1,
        }
        assert!(matches!(
            f.transport
                .call(ZoneRpcRequest::ValidateOwnerHealth {
                    source_registration_id: item.source_registration_id,
                    change: changed,
                })
                .unwrap(),
            ZoneRpcPayload::Bool { value: false }
        ));
    }
    assert!(matches!(
        f.transport
            .call(ZoneRpcRequest::ValidateOwnerHealth {
                source_registration_id: item.source_registration_id + 1,
                change: original.clone(),
            })
            .unwrap(),
        ZoneRpcPayload::Bool { value: false }
    ));
    let mut missing = f.transport.clone();
    missing.session_id = "missing-health-session".into();
    assert!(matches!(
        missing
            .call(ZoneRpcRequest::ValidateOwnerHealth {
                source_registration_id: item.source_registration_id,
                change: original.clone(),
            })
            .unwrap(),
        ZoneRpcPayload::Bool { value: false }
    ));
    let mut unauthenticated = f.transport.clone();
    unauthenticated.auth_token = Some("wrong-token".into());
    assert!(unauthenticated
        .call(ZoneRpcRequest::ValidateOwnerHealth {
            source_registration_id: item.source_registration_id,
            change: original,
        })
        .is_err());
    assert_eq!(f.server.session_count(), before_sessions);
    assert_eq!(
        f.server.journal.lock().unwrap().entries.len(),
        before_journal
    );
    assert_eq!(f.transport.world_snapshot().unwrap(), before_snapshot);
}

#[test]
fn owner_health_rpc_retained_expired_frame_keeps_sequence_until_normal_ack() {
    let mut f = Fixture::new();
    let item = f.drug_health();
    assert!(f.validate(&item));
    let snapshot = f.transport.world_snapshot().unwrap();
    let owner = snapshot
        .entities
        .iter()
        .find(|e| e.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
        .unwrap();
    f.execute(WorldCommand::ApplyHandoffTransform {
        position: Point {
            x: owner.x,
            y: owner.y,
        },
        direction: MirDirection::Right,
        hp: Some(0),
        mp: None,
    });
    f.execute(WorldCommand::ClientPacket(ClientPacket::TownRevive));
    assert!(f
        .transport
        .world_snapshot()
        .unwrap()
        .player_hp
        .is_some_and(|hp| hp > 0));
    // No refreshed local cursor is supplied to this final validator. The
    // authenticated request reads the current Host admission and life.
    let validator = RemoteOwnerHealthValidator {
        transport: f.transport.clone(),
        endpoint: item.source_endpoint,
        source_registration_id: item.source_registration_id,
        generation: 1,
        current_generation: Arc::new(AtomicU64::new(1)),
        stop: Arc::new(AtomicBool::new(false)),
    };
    assert!(!validator
        .validate(item.owner_health.as_ref().unwrap())
        .unwrap());
    let replay = f.transport.poll_outbounds(f.ack, 128).unwrap();
    let retained = replay
        .items
        .iter()
        .find(|entry| entry.sequence == item.sequence)
        .unwrap();
    assert_eq!(retained.owner_health, item.owner_health);
    assert_eq!(retained.source_registration_id, item.source_registration_id);
    let acknowledged = replay.items.last().unwrap().sequence;
    let next = f.transport.poll_outbounds(acknowledged, 128).unwrap();
    assert!(next.items.iter().all(|entry| entry.sequence > acknowledged));
}

#[test]
fn owner_health_rpc_changed_map_rebuilds_host_registration_and_new_drug_still_arrives() {
    let mut f = Fixture::new();
    let old = f.drug_health();
    let key = ("health-rpc-session".to_string(), "primary".to_string());
    let hosted = f.server.sessions.lock().unwrap().get(&key).unwrap().clone();
    let previous = hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .registration_id();
    f.execute(WorldCommand::TransferMap {
        key: "crystal:3:330:270".into(),
    });
    assert_eq!(
        f.transport
            .world_snapshot()
            .unwrap()
            .map_file_name
            .as_deref(),
        Some("3")
    );
    let current = hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .registration_id();
    assert_ne!(previous, current);
    assert!(hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .is_current());
    assert!(!f.validate(&old));
    f.clear();
    let new = f.drug_health();
    assert!(f.validate(&new));
    let max_hp = f.transport.world_snapshot().unwrap().player_max_hp.unwrap();
    assert!(matches!(new.packet, ServerPacket::HealthChanged { hp, .. } if hp == max_hp));
}

#[test]
fn owner_health_rpc_old_viewport_is_skipped_without_renumbering_or_losing_social_receipts() {
    let mut f = Fixture::new();
    let mut initial = f.transport.active_character_checkpoint().unwrap().unwrap();
    initial.character.level = 60;
    initial.mp = 200;
    initial.max_mp = 200;
    initial.skill_states_json = vec![serde_json::json!({
        "key": "flamingsword", "name": "FlamingSword", "description": "",
        "level": 0, "experience": 0, "cooldown_ticks": 0, "cooldown_ends_at": 0,
    })
    .to_string()];
    f.transport
        .restore_active_character_checkpoint(&initial)
        .unwrap();
    f.clear();
    f.execute(WorldCommand::ClientPacket(ClientPacket::SpellToggle {
        spell: mir2_protocol::Spell::FlamingSword,
        toggle_state: 1,
    }));
    f.execute(WorldCommand::ClientPacket(ClientPacket::Turn {
        direction: MirDirection::Left,
    }));
    let key = ("health-rpc-session".to_string(), "primary".to_string());
    let hosted = f.server.sessions.lock().unwrap().get(&key).unwrap().clone();
    let source = hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .registration_id();
    // A prepared transport-only social receipt. This fixture does not claim
    // that an actual NPC/guild command produced the notice.
    let notice = ServerPacket::Chat {
        message: "owned retained social receipt".into(),
        chat_type: mir2_protocol::ChatType::System,
    };
    hosted
        .outbound_sender
        .blocking_send(SharedZoneLiveOutbound::new(source, notice.clone()))
        .unwrap();
    let first = f.transport.poll_outbounds(f.ack, 128).unwrap();
    let old = first
        .items
        .iter()
        .find(|item| {
            matches!(
                item.packet,
                ServerPacket::SpellToggle {
                    spell: mir2_protocol::Spell::FlamingSword,
                    can_use: true,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let social = first
        .items
        .iter()
        .find(|item| item.packet == notice)
        .unwrap()
        .clone();
    let old_location = first
        .items
        .iter()
        .find(|item| matches!(item.packet, ServerPacket::UserLocation { .. }))
        .unwrap()
        .clone();
    assert_eq!(
        first.current_source_registration_id,
        Some(old.source_registration_id)
    );
    f.execute(WorldCommand::TransferMap {
        key: "crystal:3:330:270".into(),
    });
    let retained = f.transport.poll_outbounds(f.ack, 128).unwrap();
    assert_ne!(
        retained.current_source_registration_id,
        Some(old.source_registration_id)
    );
    assert!(retained
        .items
        .iter()
        .any(|item| item.sequence == old.sequence && item.packet == old.packet));
    assert!(!source_viewport_frame_is_current(
        &old,
        retained.current_source_registration_id
    ));
    assert!(!source_viewport_frame_is_current(
        &old_location,
        retained.current_source_registration_id
    ));
    assert!(source_viewport_frame_is_current(
        &social,
        retained.current_source_registration_id
    ));
    assert!(!source_viewport_frame_is_current(&old, None));

    let (tx, mut rx) = mpsc::channel(1024);
    let remote = f
        .transport
        .register_live_outbound(SharedZoneLiveOutboundSender::single(tx))
        .unwrap()
        .unwrap();
    remote.activate();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let delivered = rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut packets = Vec::new();
            loop {
                let outbound = rx.recv().await.unwrap();
                if let Some(packet) = outbound.claim_for_send().unwrap() {
                    let found = packet == notice;
                    packets.push(packet);
                    if found {
                        return packets;
                    }
                }
            }
        })
        .await
        .unwrap()
    });
    assert!(delivered.iter().all(|packet| packet != &old.packet));
    assert!(delivered
        .iter()
        .all(|packet| packet != &old_location.packet));
    let deadline = Instant::now() + Duration::from_secs(2);
    while f.transport.outbound_acknowledged.load(Ordering::Acquire) < social.sequence {
        assert!(
            Instant::now() < deadline,
            "skipped old frame must not leave an ACK hole"
        );
        thread::yield_now();
    }
    let acknowledged = f.transport.outbound_acknowledged.load(Ordering::Acquire);
    assert!(f
        .transport
        .poll_outbounds(acknowledged, 128)
        .unwrap()
        .items
        .iter()
        .all(|item| item.sequence > acknowledged));
    drop(remote);
}

#[test]
fn owner_health_rpc_source_overflow_requires_resync_and_cancels_remote_transport() {
    let f = Fixture::new();
    let key = ("health-rpc-session".to_string(), "primary".to_string());
    let original = f.server.sessions.lock().unwrap().get(&key).unwrap().clone();
    let small = Arc::new(ZoneHostSession::new(original.hosted.clone(), 1));
    small.refresh_live_registration(true).unwrap();
    f.server.sessions.lock().unwrap().insert(key, small.clone());
    let registration_id = {
        let registration = small.live_registration.lock().unwrap();
        let registration = registration.as_ref().unwrap();
        registration.prepare_owner_health_backpressure_for_test();
        assert!(registration.resync_required());
        registration.registration_id()
    };
    let fault = small.refresh_live_registration(false).unwrap_err();
    assert_eq!(fault.code, "outbound_gap");
    assert_eq!(
        small
            .live_registration
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .registration_id(),
        registration_id
    );
    assert!(f
        .transport
        .poll_outbounds(f.ack, 128)
        .unwrap_err()
        .contains("outbound_gap"));
    let (tx, mut rx) = mpsc::channel(4);
    let (signal, latest) = tokio::sync::watch::channel(0);
    let remote = f
        .transport
        .register_live_outbound(
            SharedZoneLiveOutboundSender::single(tx).with_overload_signal(signal),
        )
        .unwrap()
        .unwrap();
    remote.activate();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let sentinel = rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap()
    });
    assert_eq!(sentinel.registration_id(), remote.registration_id());
    assert!(sentinel.claim_for_send().is_err());
    assert_eq!(*latest.borrow(), remote.registration_id());
    drop(remote);
}

#[test]
fn owner_health_rpc_concurrent_refresh_keeps_the_installed_registration_current() {
    let mut f = Fixture::new();
    let key = ("health-rpc-session".to_string(), "primary".to_string());
    let hosted = f.server.sessions.lock().unwrap().get(&key).unwrap().clone();
    let start = Arc::new(std::sync::Barrier::new(8));
    let workers = (0..8)
        .map(|_| {
            let hosted = hosted.clone();
            let start = start.clone();
            thread::spawn(move || {
                start.wait();
                for _ in 0..8 {
                    hosted.refresh_live_registration(true).unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    assert!(hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .is_current());
    f.clear();
    let item = f.drug_health();
    assert!(f.validate(&item));
    assert_eq!(
        item.source_registration_id,
        hosted
            .live_registration
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .registration_id()
    );
}

#[test]
fn owner_health_rpc_drain_never_serializes_a_private_cancellation_sentinel() {
    let f = Fixture::new();
    let key = ("health-rpc-session".to_string(), "primary".to_string());
    let hosted = f.server.sessions.lock().unwrap().get(&key).unwrap().clone();
    let registration_id = hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .registration_id();
    let before = hosted.outbox.lock().unwrap().last_issued_sequence;
    // Prepared cancellation arriving after the registration preflight. The
    // actual sender constructs the private sentinel, not a health event.
    hosted
        .outbound_sender
        .cancel_registration(registration_id, Arc::new(AtomicBool::new(false)));
    assert!(!hosted
        .live_registration
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .resync_required());
    assert!(f
        .transport
        .poll_outbounds(f.ack, 128)
        .unwrap_err()
        .contains("outbound_gap"));
    assert_eq!(hosted.outbox.lock().unwrap().last_issued_sequence, before);
}
