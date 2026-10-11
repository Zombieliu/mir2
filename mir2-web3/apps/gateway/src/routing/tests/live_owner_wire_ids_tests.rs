//! Ordinary Gateway login/StartGame and socket registration, followed by
//! trusted shared Zone outbounds. This tests the actual bounded live delivery
//! path, including retained global backlog; no pure normalizer is called.
use super::*;
use crate::routing::{
    SharedZoneLiveOutbound, SharedZoneLiveOutboundSender, ZoneLiveOutboundRegistration,
};
use mir2_simulation::ZoneOutbound;

const PET: u32 = 7_600_001;
const WILD: u32 = 7_600_002;

struct Scene {
    owner: GatewaySession,
    observer: GatewaySession,
    state: Arc<Mutex<SharedInProcessZoneState>>,
    key: ZonePresenceKey,
    observer_key: ZonePresenceKey,
    owner_id: u32,
}

fn registry() -> (ZoneRegistry, Arc<SharedInProcessZoneRuntimeFactory>) {
    // Keep the real single writer and ingress, with no autonomous clock racing
    // these explicitly delivered socket/backpressure boundary assertions.
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_secs(3600),
        BTreeMap::new(),
    ));
    let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
    (registry, factory)
}

impl Scene {
    fn new() -> Self {
        let (registry, factory) = registry();
        let config = GatewayConfig::default();
        let mut owner = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut observer = GatewaySession::new_with_zone_registry(config, &registry);
        start_new_character(&mut owner, "wire-owner", "WireOwner");
        start_new_character(&mut observer, "wire-observer", "WireObserver");
        let key = ZonePresenceKey::from_identity(&owner.active_identity().unwrap());
        let observer_key = ZonePresenceKey::from_identity(&observer.active_identity().unwrap());
        let state = factory
            .resources_for_zone(owner.zone_id())
            .zone_state
            .clone();
        let owner_id = {
            let mut state = state.lock().unwrap();
            let owner_id = state.players[&key].zone_object_id;
            assert_ne!(owner_id, 1000);
            assert_eq!(state.players[&key].owner_local_object_id, 1000);
            assert_eq!(state.players[&observer_key].owner_local_object_id, 1000);
            state.take_pending_zone_packets(&key);
            state.take_pending_zone_packets(&observer_key);
            owner_id
        };
        let self_entity = owner
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        assert_eq!(self_entity.object_id, 1000);
        Self {
            owner,
            observer,
            state,
            key,
            observer_key,
            owner_id,
        }
    }

    fn live(
        session: &GatewaySession,
        capacity: usize,
    ) -> (
        Box<dyn ZoneLiveOutboundRegistration>,
        tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        let registration = session
            .register_zone_live_outbound(SharedZoneLiveOutboundSender::single(sender))
            .unwrap()
            .expect("ordinary active Gateway owner must register");
        registration.activate();
        (registration, receiver)
    }
}

fn packets(owner: u32) -> Vec<ServerPacket> {
    vec![
        ServerPacket::DamageIndicator {
            object_id: owner,
            damage: 13,
            damage_type: 0,
        },
        ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: owner,
                attacker_id: WILD,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Down,
            },
        },
        ServerPacket::ObjectMonster {
            info: MonsterInfo {
                object_id: PET,
                name: "BoneFamiliar".into(),
                name_colour_argb: -1,
                location: Point { x: 331, y: 270 },
                image: 1,
                direction: MirDirection::Down,
                effect: 0,
                ai: 4,
                light: 0,
                dead: false,
                skeleton: false,
                poison: 0,
                hidden: false,
                shock_time: 0,
                binding_shot_center: false,
                extra: false,
                extra_byte: 0,
                master_object_id: owner,
                rarity: 0,
                buffs: vec![],
            },
        },
    ]
}

fn drain(receiver: &mut tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    while let Ok(outbound) = receiver.try_recv() {
        packets.push(outbound.into_packet());
    }
    packets
}

fn retained_packets(state: &SharedInProcessZoneState, key: &ZonePresenceKey) -> Vec<ServerPacket> {
    // Owner presentation now shares the ordered health FIFO. Inspect both
    // actual queues so these assertions still require the exact global packet
    // sequence, rather than assuming it resides in the legacy queue.
    let mut packets = state.pending_zone_owner_health.get(key).into_iter()
        .flatten().map(|presentation| match presentation {
            super::super::owner_health::PendingOwnerPresentation::Packet(packet) => packet.clone(),
            super::super::owner_health::PendingOwnerPresentation::Health(_) =>
                panic!("quiet wire fixture must not introduce a health mutation"),
        }).collect::<Vec<_>>();
    packets.extend(state.pending_zone_packets.get(key).into_iter().flatten().cloned());
    packets
}

fn assert_wire(packets: &[ServerPacket], owner: u32) {
    assert_eq!(packets.len(), 3, "one exact source sequence: {packets:?}");
    assert!(
        matches!(&packets[0], ServerPacket::DamageIndicator { object_id, damage: 13, .. }
        if *object_id == owner)
    );
    assert!(matches!(&packets[1], ServerPacket::ObjectStruck { info }
        if info.object_id == owner && info.attacker_id == WILD));
    assert!(matches!(&packets[2], ServerPacket::ObjectMonster { info }
        if info.object_id == PET && info.master_object_id == owner));
}

#[test]
fn live_owner_wire_ids_registered_gateway_owner_rebases_self_and_pet_master_only() {
    let scene = Scene::new();
    let (owner_registration, mut owner_receiver) = Scene::live(&scene.owner, 8);
    let (observer_registration, mut observer_receiver) = Scene::live(&scene.observer, 8);
    drain(&mut owner_receiver);
    drain(&mut observer_receiver);
    let original = packets(scene.owner_id);
    {
        let mut state = scene.state.lock().unwrap();
        assert_eq!(
            state.live_zone_outbounds[&scene.key].owner_wire_ids,
            Some((scene.owner_id, 1000))
        );
        let sessions =
            [&scene.key, &scene.observer_key].map(|key| state.zone_sessions[key].clone());
        let returned = state
            .dispatch_zone_outbounds(
                vec![ZoneOutbound::ToMany {
                    session_ids: sessions.to_vec(),
                    packets: original.clone(),
                }],
                None,
            )
            .0;
        assert!(returned.is_empty());
    }
    let owner = drain(&mut owner_receiver);
    let observer = drain(&mut observer_receiver);
    assert_wire(&owner, 1000);
    assert_eq!(
        observer, original,
        "observer sees the shared owner's global IDs"
    );
    assert!(drain(&mut owner_receiver).is_empty());
    assert_ne!(
        owner_registration.registration_id(),
        observer_registration.registration_id()
    );
    assert_eq!(
        scene.state.lock().unwrap().players[&scene.key]
            .entity
            .object_id,
        scene.owner_id
    );
}

#[test]
fn live_owner_wire_ids_full_channel_retains_global_backlog_until_single_wire_projection() {
    let scene = Scene::new();
    let (_registration, mut receiver) = Scene::live(&scene.owner, 1);
    drain(&mut receiver);
    let original = packets(scene.owner_id);
    scene
        .state
        .lock()
        .unwrap()
        .queue_zone_packets(scene.key.clone(), original.clone());
    let first = receiver.try_recv().unwrap().into_packet();
    assert!(matches!(
        first,
        ServerPacket::DamageIndicator {
            object_id: 1000,
            damage: 13,
            ..
        }
    ));
    assert_eq!(
        retained_packets(&scene.state.lock().unwrap(), &scene.key),
        original[1..]
    );
    let mut actual = vec![first];
    for index in 1..3 {
        scene
            .state
            .lock()
            .unwrap()
            .retry_pending_realtime_zone_outbounds();
        let outbound = receiver.try_recv().unwrap();
        actual.push(outbound.into_packet());
        let state = scene.state.lock().unwrap();
        let pending = retained_packets(&state, &scene.key);
        assert_eq!(
            pending,
            original[index + 1..],
            "retained packets never contain local IDs"
        );
    }
    assert_wire(&actual, 1000);
    scene
        .state
        .lock()
        .unwrap()
        .retry_pending_realtime_zone_outbounds();
    assert!(
        receiver.try_recv().is_err(),
        "retry cannot deliver a source twice"
    );
}

#[test]
fn live_owner_wire_ids_closed_channel_keeps_global_packets_for_new_registration() {
    let scene = Scene::new();
    let (old_registration, receiver) = Scene::live(&scene.owner, 8);
    drop(receiver);
    let original = packets(scene.owner_id);
    scene
        .state
        .lock()
        .unwrap()
        .queue_zone_packets(scene.key.clone(), original.clone());
    {
        let state = scene.state.lock().unwrap();
        assert!(!state.live_zone_outbounds.contains_key(&scene.key));
        assert_eq!(state.pending_zone_packets[&scene.key], original);
    }
    let (new_registration, mut receiver) = Scene::live(&scene.owner, 8);
    assert_ne!(
        old_registration.registration_id(),
        new_registration.registration_id()
    );
    drop(old_registration);
    assert_eq!(
        scene.state.lock().unwrap().live_zone_outbounds[&scene.key].registration_id,
        new_registration.registration_id(),
        "old socket closure cannot erase the replacement"
    );
    assert_wire(&drain(&mut receiver), 1000);
    assert!(!scene
        .state
        .lock()
        .unwrap()
        .pending_zone_packets
        .contains_key(&scene.key));
}

#[test]
fn live_owner_wire_ids_bootstrap_without_startgame_has_no_trusted_owner_mapping() {
    let (registry, factory) = registry();
    let session = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
    let registration = session
        .register_zone_live_outbound(SharedZoneLiveOutboundSender::single(sender))
        .unwrap();
    if let Some(registration) = registration {
        registration.activate();
    }
    let state = factory
        .resources_for_zone(session.zone_id())
        .zone_state
        .clone();
    assert!(state.lock().unwrap().players.is_empty());
    assert!(state.lock().unwrap().live_zone_outbounds.is_empty());
    assert!(receiver.try_recv().is_err());
}

#[test]
fn live_owner_wire_ids_normal_logout_rejoin_replaces_old_owner_mapping() {
    let mut scene = Scene::new();
    let (old_registration, mut old_receiver) = Scene::live(&scene.owner, 8);
    drain(&mut old_receiver);
    let character = scene.owner.active_identity().unwrap().character_index;
    assert!(scene
        .owner
        .handle_packet(ClientPacket::LogOut)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(!scene
        .state
        .lock()
        .unwrap()
        .live_zone_outbounds
        .contains_key(&scene.key));
    scene.owner.handle_packet(ClientPacket::StartGame {
        character_index: character,
    });
    let new_id = scene.state.lock().unwrap().players[&scene.key].zone_object_id;
    assert_ne!(
        new_id, scene.owner_id,
        "new join has a new global Human object"
    );
    let (new_registration, mut new_receiver) = Scene::live(&scene.owner, 8);
    drain(&mut new_receiver);
    drop(old_registration);
    {
        let mut state = scene.state.lock().unwrap();
        assert_eq!(
            state.live_zone_outbounds[&scene.key].owner_wire_ids,
            Some((new_id, 1000))
        );
        assert_eq!(
            state.live_zone_outbounds[&scene.key].registration_id,
            new_registration.registration_id()
        );
        state.queue_zone_packets(scene.key.clone(), packets(scene.owner_id));
        state.queue_zone_packets(scene.key.clone(), packets(new_id));
    }
    let actual = drain(&mut new_receiver);
    assert_eq!(actual.len(), 6);
    assert_wire(&actual[..3], scene.owner_id);
    assert_wire(&actual[3..], 1000);
    assert!(
        old_receiver.try_recv().is_err(),
        "old registration receives no new world packet"
    );
}
