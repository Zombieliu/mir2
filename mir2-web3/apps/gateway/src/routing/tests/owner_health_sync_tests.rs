//! Actual shared registration/queue/final-send paths, with prepared pool states.
use super::*;
use crate::routing::{
    SharedZoneLiveOutbound, SharedZoneLiveOutboundRegistration, SharedZoneLiveOutboundSender,
};
use crate::routing::{ZoneLiveOutboundRegistration, MAX_PENDING_ZONE_PACKETS_PER_PLAYER};
use std::sync::atomic::AtomicBool;

struct Owner {
    runtime: SharedInProcessZoneSessionRuntime,
    shared: Arc<Mutex<SharedInProcessZoneState>>,
    key: ZonePresenceKey,
    session: SessionId,
    registration: SharedZoneLiveOutboundRegistration,
    rx: tokio::sync::mpsc::Receiver<SharedZoneLiveOutbound>,
}

impl Owner {
    fn new(capacity: usize) -> Self {
        Self::with_config(capacity, GatewayConfig::default())
    }

    fn with_config(capacity: usize, config: GatewayConfig) -> Self {
        let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        shared.lock().unwrap().experience_authority = Some(config);
        let mut runtime = shared_session_runtime(shared.clone());
        start_new_runtime(&mut runtime, "owner-health-routing", "HealthOwner");
        let key = runtime.current_presence_key().unwrap();
        let session = runtime.current_zone_session_id().unwrap();
        let (sender, rx) = tokio::sync::mpsc::channel(capacity);
        let registration = runtime
            .movement_ingress
            .register_live_outbound(SharedZoneLiveOutboundSender::single(sender))
            .unwrap()
            .unwrap();
        registration.activate();
        let mut owner = Self {
            runtime,
            shared,
            key,
            session,
            registration,
            rx,
        };
        owner.flush();
        owner
    }

    fn commit(&self, hp: i32, mp: i32, dead: bool) {
        let mut state = self.shared.lock().unwrap();
        let out = state
            .zone_manager
            .commit_owner_vitals(&self.session, hp, 100, mp, dead);
        // Current command results cannot strip the capability into a raw packet.
        let direct = state.dispatch_zone_outbounds(out, Some(&self.key)).0;
        assert!(!direct
            .iter()
            .any(|p| matches!(p, ServerPacket::HealthChanged { .. })));
    }

    fn drain(&mut self) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        while let Ok(outbound) = self.rx.try_recv() {
            if let Some(packet) = outbound.claim_for_send().unwrap() {
                packets.push(packet);
            }
        }
        packets
    }

    fn flush(&mut self) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        for _ in 0..16 {
            packets.extend(self.drain());
            let mut state = self.shared.lock().unwrap();
            state.retry_pending_realtime_zone_outbounds();
            if !state.pending_zone_owner_health.contains_key(&self.key)
                && !state.pending_zone_packets.contains_key(&self.key)
            {
                drop(state);
                packets.extend(self.drain());
                return packets;
            }
        }
        panic!("bounded fixture presentation failed to drain");
    }
}

fn pools(packets: &[ServerPacket]) -> Vec<(i32, i32)> {
    packets
        .iter()
        .filter_map(|p| match p {
            ServerPacket::HealthChanged { hp, mp } => Some((*hp, *mp)),
            _ => None,
        })
        .collect()
}

#[test]
fn owner_health_routing_exact_hp_then_mp_uses_only_owner_channel() {
    let mut owner = Owner::new(64);
    owner.commit(10, 20, false);
    owner.drain();
    let mut observer = shared_session_runtime(owner.shared.clone());
    start_new_runtime(&mut observer, "health-observer", "HealthPeer");
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let _peer_registration = observer
        .movement_ingress
        .register_live_outbound(SharedZoneLiveOutboundSender::single(tx))
        .unwrap()
        .unwrap();
    owner.drain();
    while rx.try_recv().is_ok() {}
    owner.commit(11, 21, false);
    assert_eq!(pools(&owner.drain()), vec![(11, 20), (11, 21)]);
    while let Ok(outbound) = rx.try_recv() {
        assert!(!matches!(
            outbound.into_packet(),
            ServerPacket::HealthChanged { .. }
        ));
    }
}

#[test]
fn owner_health_routing_same_life_history_survives_lethal_commit() {
    let mut owner = Owner::new(64);
    owner.commit(10, 20, false);
    owner.drain();
    owner.commit(9, 19, false);
    owner.commit(0, 19, true);
    assert_eq!(pools(&owner.drain()), vec![(9, 20), (9, 19), (0, 19)]);
}

#[test]
fn owner_health_routing_queued_old_life_is_rejected_after_revive_without_cache_tick() {
    let mut owner = Owner::new(64);
    owner.commit(10, 20, false);
    owner.drain();
    owner.commit(0, 20, true);
    let old = owner.rx.try_recv().unwrap();
    owner.commit(50, 20, false);
    assert!(old.claim_for_send().unwrap().is_none());
    assert_eq!(pools(&owner.drain()), vec![(50, 20)]);
}

#[test]
fn owner_health_routing_full_channel_preserves_death_then_health_then_revived_fifo() {
    let mut owner = Owner::new(1);
    owner.commit(10, 20, false);
    owner.flush();
    owner.commit(9, 20, false); // Fill the real normal channel.
    {
        let mut state = owner.shared.lock().unwrap();
        // These prepared control packets test retry ordering; they do not
        // grant death/revival authority or perform an economic settlement.
        state.queue_zone_packets(
            owner.key.clone(),
            vec![ServerPacket::Death {
                location: Point { x: 1, y: 1 },
                direction: MirDirection::Right,
            }],
        );
    }
    owner.commit(8, 20, false);
    owner
        .shared
        .lock()
        .unwrap()
        .queue_zone_packets(owner.key.clone(), vec![ServerPacket::Revived]);
    let mut packets = owner.drain();
    for _ in 0..3 {
        owner
            .shared
            .lock()
            .unwrap()
            .retry_pending_realtime_zone_outbounds();
        packets.extend(owner.drain());
    }
    assert_eq!(packets.len(), 4);
    assert!(matches!(
        packets[0],
        ServerPacket::HealthChanged { hp: 9, .. }
    ));
    assert!(matches!(packets[1], ServerPacket::Death { .. }));
    assert!(matches!(
        packets[2],
        ServerPacket::HealthChanged { hp: 8, .. }
    ));
    assert!(matches!(packets[3], ServerPacket::Revived));
}

#[test]
fn owner_health_routing_duplicate_reverse_sequence_and_payload_tamper_fail_closed() {
    let mut owner = Owner::new(64);
    owner.commit(10, 20, false);
    owner.drain();
    owner.commit(9, 19, false);
    let earlier = owner.rx.try_recv().unwrap();
    let mut later = owner.rx.try_recv().unwrap();
    let proof = later.owner_health().unwrap().clone();
    assert!(later.claim_for_send().unwrap().is_some());
    assert!(earlier.claim_for_send().unwrap().is_none());
    let duplicate = SharedZoneLiveOutbound::new(
        owner.registration.registration_id(),
        ServerPacket::HealthChanged { hp: 9, mp: 19 },
    )
    .with_owner_health(proof.clone());
    assert!(duplicate.claim_for_send().unwrap().is_none());
    later = SharedZoneLiveOutbound::new(
        owner.registration.registration_id(),
        ServerPacket::HealthChanged { hp: 100, mp: 100 },
    )
    .with_owner_health(proof);
    assert!(later.claim_for_send().is_err());
}

#[test]
fn owner_health_routing_replaced_registration_and_cold_state_reject_old_capability() {
    let mut owner = Owner::new(64);
    owner.commit(10, 20, false);
    owner.drain();
    owner.commit(9, 20, false);
    let old = owner.rx.try_recv().unwrap();
    let (sender, mut next_rx) = tokio::sync::mpsc::channel(64);
    let replacement = owner
        .runtime
        .movement_ingress
        .register_live_outbound(SharedZoneLiveOutboundSender::single(sender))
        .unwrap()
        .unwrap();
    assert!(!owner.registration.is_current());
    assert!(replacement.is_current());
    assert!(old.claim_for_send().unwrap().is_none());
    owner.commit(8, 20, false);
    let queued = next_rx.try_recv().unwrap();
    let checkpoint = owner.shared.lock().unwrap().checkpoint().unwrap();
    *owner.shared.lock().unwrap() = SharedInProcessZoneState::restore(checkpoint).unwrap();
    assert!(queued.claim_for_send().unwrap().is_none());
    assert!(owner
        .shared
        .lock()
        .unwrap()
        .pending_zone_owner_health
        .is_empty());
}

#[test]
fn owner_health_routing_overload_closes_instead_of_silently_truncating_tcp_health() {
    let mut owner = Owner::new(1);
    owner.commit(10, 20, false);
    owner.flush();
    owner.commit(9, 20, false);
    for index in 0..=MAX_PENDING_ZONE_PACKETS_PER_PLAYER {
        owner.commit(if index % 2 == 0 { 8 } else { 9 }, 20, false);
    }
    let queued = owner.rx.try_recv().unwrap();
    assert!(queued.claim_for_send().is_err());
    assert!(!owner.registration.is_current());
    assert!(owner
        .shared
        .lock()
        .unwrap()
        .pending_zone_owner_health
        .is_empty());
}

#[test]
fn owner_health_routing_normal_drug_tick_starts_at_latest_zone_hp_and_keeps_recovery() {
    let mut owner = Owner::with_config(
        256,
        GatewayConfig::default()
            .with_crystal_world_runtime()
            .with_platinum_176_profile(),
    );
    let mp = owner
        .runtime
        .inner
        .local_player_vitals_snapshot()
        .player_mp
        .unwrap();
    let max_hp = owner
        .runtime
        .inner
        .local_player_vitals_snapshot()
        .player_max_hp
        .unwrap();
    assert!(
        max_hp > 10 && max_hp <= 20,
        "native level-one Warrior ceiling"
    );
    let potion = owner
        .runtime
        .inner
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.name == "(HP)DrugSmall")
        .unwrap()
        .unique_id;
    let mut state = owner.shared.lock().unwrap();
    let changes = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 10, max_hp, mp, false);
    state.dispatch_zone_outbounds(changes, Some(&owner.key));
    drop(state);
    owner.drain();
    // The private mirror is still older. Ordinary UseItem must first see Zone HP.
    let used = owner
        .runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: potion,
            grid: mir2_protocol::MirGridType::Inventory,
        }))
        .unwrap();
    assert!(used.iter().any(
        |packet| matches!(packet, ServerPacket::UseItem { unique_id, success: true, .. }
        if *unique_id == potion)
    ));
    owner.drain();
    owner.runtime.execute(WorldCommand::Tick).unwrap();
    let packets = owner.drain();
    assert_eq!(pools(&packets), vec![(max_hp, mp)]);
    assert_eq!(
        owner
            .shared
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&owner.session)
            .unwrap()
            .0,
        max_hp
    );
    assert_eq!(
        owner.runtime.inner.local_player_vitals_snapshot().player_hp,
        Some(max_hp)
    );
}

#[test]
fn owner_health_routing_social_receipts_survive_bootstrap_and_checkpoint() {
    let mut owner = Owner::new(1);
    owner.commit(10, 20, false);
    owner.flush();
    owner.commit(9, 20, false); // Occupy the normal channel.
    let receipts = vec![
        ServerPacket::MentorRequest {
            name: "Teacher".into(),
            level: 40,
        },
        ServerPacket::GuildInvite {
            name: "Guild".into(),
        },
    ];
    let mut state = owner.shared.lock().unwrap();
    state.queue_zone_packets(owner.key.clone(), receipts.clone());
    let changes = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 8, 100, 20, false);
    state.dispatch_zone_outbounds(changes, Some(&owner.key));
    assert_eq!(state.pending_zone_packets.get(&owner.key), Some(&receipts));
    assert!(state.pending_zone_owner_health.contains_key(&owner.key));
    state.begin_zone_bootstrap(&owner.key);
    assert_eq!(state.pending_zone_packets.get(&owner.key), Some(&receipts));
    assert!(!state.pending_zone_owner_health.contains_key(&owner.key));
    let restored = SharedInProcessZoneState::restore(state.checkpoint().unwrap()).unwrap();
    assert_eq!(
        restored.pending_zone_packets.get(&owner.key),
        Some(&receipts)
    );
    assert!(restored.pending_zone_owner_health.is_empty());
}

#[test]
fn owner_health_routing_unchanged_mirror_does_not_advance_an_extra_personal_tick() {
    let mut owner = Owner::new(64);
    let before = owner.runtime.inner.world_snapshot().tick;
    owner
        .runtime
        .execute_shared_personal_vital_command(WorldCommand::Tick, true)
        .unwrap();
    let after = owner.runtime.inner.world_snapshot().tick;
    let mut baseline = Owner::new(64);
    let baseline_before = baseline.runtime.inner.world_snapshot().tick;
    baseline.runtime.inner.tick_shared_zone_personal_state();
    let baseline_after = baseline.runtime.inner.world_snapshot().tick;
    assert!(baseline_after > baseline_before);
    assert_eq!(after - before, baseline_after - baseline_before);
}

#[test]
fn owner_health_routing_late_old_cancellation_cannot_replace_current_signal() {
    let (tx, _rx) = tokio::sync::mpsc::channel(4);
    let (signal, latest) = tokio::sync::watch::channel(0);
    let sender = SharedZoneLiveOutboundSender::single(tx).with_overload_signal(signal);
    sender.cancel_registration(9, Arc::new(AtomicBool::new(false)));
    sender.cancel_registration(8, Arc::new(AtomicBool::new(false)));
    assert_eq!(*latest.borrow(), 9);
}

#[test]
fn owner_health_routing_cast_and_toggle_cannot_overtake_mp_on_the_live_channel() {
    let mut owner = Owner::new(1);
    owner.commit(10, 20, false);
    owner.flush();
    let mut current = Vec::new();
    owner.shared.lock().unwrap().collect_current_zone_packets(
        &owner.key,
        vec![ServerPacket::SpellToggle {
            object_id: 1,
            spell: mir2_protocol::Spell::FlamingSword,
            can_use: true,
        }],
        &mut current,
    );
    owner.commit(10, 19, false);
    owner.shared.lock().unwrap().collect_current_zone_packets(
        &owner.key,
        vec![ServerPacket::Magic {
            spell: mir2_protocol::Spell::Healing,
            target_id: 0,
            target: Point { x: 1, y: 1 },
            cast: true,
            level: 0,
            secondary_target_ids: Vec::new(),
        }],
        &mut current,
    );
    assert!(current.is_empty());
    let packets = owner.flush();
    assert_eq!(packets.len(), 3);
    assert!(matches!(packets[0], ServerPacket::SpellToggle { .. }));
    assert!(matches!(
        packets[1],
        ServerPacket::HealthChanged { hp: 10, mp: 19 }
    ));
    assert!(matches!(packets[2], ServerPacket::Magic { .. }));
}

fn prepared_normal_potion() -> Owner {
    let mut owner = Owner::with_config(64, GatewayConfig::default().with_platinum_176_profile());
    let snapshot = owner.runtime.inner.world_snapshot();
    let potion = snapshot
        .inventory_items
        .iter()
        .find(|i| i.name == "(HP)DrugSmall")
        .unwrap()
        .unique_id;
    let mut state = owner.shared.lock().unwrap();
    let out = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 10, 18, 14, false);
    state.dispatch_zone_outbounds(out, Some(&owner.key));
    drop(state);
    owner
        .runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: potion,
            grid: mir2_protocol::MirGridType::Inventory,
        }))
        .unwrap();
    owner.flush();
    owner
}

#[test]
fn owner_health_routing_personal_potion_delta_preserves_a_concurrent_zone_hit() {
    let mut owner = prepared_normal_potion();
    let basis = owner
        .runtime
        .prepare_shared_personal_vital_command()
        .unwrap()
        .unwrap();
    let mut packets = owner
        .runtime
        .inner
        .try_tick_shared_zone_personal_state()
        .unwrap();
    assert_eq!(
        owner.runtime.inner.local_player_vitals_snapshot().player_hp,
        Some(18)
    );
    // Explicitly prepared interleaving of a committed shared HP operation.
    // This does not claim a natural monster combat encounter.
    let mut state = owner.shared.lock().unwrap();
    let out = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 7, 18, 14, false);
    state.dispatch_zone_outbounds(out, Some(&owner.key));
    drop(state);
    owner
        .runtime
        .commit_shared_personal_vital_command(basis, &mut packets)
        .unwrap();
    assert_eq!(
        owner
            .shared
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&owner.session),
        Some((15, 18, 14))
    );
    assert_eq!(
        owner.runtime.inner.local_player_vitals_snapshot().player_hp,
        Some(15)
    );
    assert_eq!(pools(&owner.flush()), vec![(7, 14), (15, 14)]);
}

#[test]
fn owner_health_routing_personal_potion_cannot_revive_a_concurrent_death_or_new_life() {
    for revive in [false, true] {
        let mut owner = prepared_normal_potion();
        let basis = owner
            .runtime
            .prepare_shared_personal_vital_command()
            .unwrap()
            .unwrap();
        let mut packets = owner
            .runtime
            .inner
            .try_tick_shared_zone_personal_state()
            .unwrap();
        let mut state = owner.shared.lock().unwrap();
        let out = state
            .zone_manager
            .commit_owner_vitals(&owner.session, 0, 18, 14, true);
        state.dispatch_zone_outbounds(out, Some(&owner.key));
        if revive {
            let out = state
                .zone_manager
                .commit_owner_vitals(&owner.session, 5, 18, 14, false);
            state.dispatch_zone_outbounds(out, Some(&owner.key));
        }
        let life = state
            .zone_manager
            .owner_health_cursor(&owner.session)
            .unwrap()
            .life_generation;
        drop(state);
        owner
            .runtime
            .commit_shared_personal_vital_command(basis, &mut packets)
            .unwrap();
        let hp = if revive { 5 } else { 0 };
        assert_eq!(
            owner
                .shared
                .lock()
                .unwrap()
                .zone_manager
                .player_vitals(&owner.session),
            Some((hp, 18, 14))
        );
        assert_eq!(
            owner
                .shared
                .lock()
                .unwrap()
                .zone_manager
                .owner_health_cursor(&owner.session)
                .unwrap()
                .life_generation,
            life
        );
        assert_eq!(
            owner.runtime.inner.local_player_vitals_snapshot().player_hp,
            Some(hp)
        );
        assert!(pools(&owner.flush()).iter().all(|(hp, _)| *hp != 18));
    }
}

#[test]
fn owner_health_routing_repository_admission_can_acquire_zone_while_personal_tick_waits() {
    let mut owner = prepared_normal_potion();
    let config = owner.runtime.inner.shared_mentor_config().unwrap();
    let shared = owner.shared.clone();
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let repository = std::thread::spawn(move || {
        config.with_shared_zone_experience_admission(|_| {
            held_tx.send(()).unwrap();
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            // Let the ordinary Tick reach its repository transaction. A failed
            // lock probe releases the Store so even a regression cannot deadlock
            // or leave the test worker behind.
            std::thread::sleep(Duration::from_millis(80));
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if shared.try_lock().is_ok() {
                    return Ok(true);
                }
                if Instant::now() >= deadline {
                    return Ok(false);
                }
                std::thread::yield_now();
            }
        })
    });
    held_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let tick = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        owner
            .runtime
            .execute_shared_personal_vital_command(WorldCommand::Tick, true)
            .unwrap();
        owner.runtime.inner.local_player_vitals_snapshot().player_hp
    });
    assert!(
        repository.join().unwrap().unwrap(),
        "personal Tick held Zone while waiting for AccountStore"
    );
    assert_eq!(tick.join().unwrap(), Some(18));
}

#[test]
fn owner_health_routing_metadata_tail_cannot_overwrite_a_later_shared_death() {
    let mut owner = prepared_normal_potion();
    assert_eq!(
        owner.runtime.inner.local_player_vitals_snapshot().player_hp,
        Some(10)
    );
    let mut state = owner.shared.lock().unwrap();
    let out = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 0, 18, 14, true);
    state.dispatch_zone_outbounds(out, Some(&owner.key));
    let life = state
        .zone_manager
        .owner_health_cursor(&owner.session)
        .unwrap()
        .life_generation;
    drop(state);
    // Prepared interleaving at the exact metadata-tail method used by ordinary
    // execute. The private projection is deliberately still alive at HP10.
    owner.runtime.sync_zone_snapshot_preserving_owner_vitals();
    let state = owner.shared.lock().unwrap();
    assert_eq!(
        state.zone_manager.player_vitals(&owner.session),
        Some((0, 18, 14))
    );
    assert_eq!(
        state.zone_manager.player_is_dead(&owner.session),
        Some(true)
    );
    assert_eq!(
        state
            .zone_manager
            .owner_health_cursor(&owner.session)
            .unwrap()
            .life_generation,
        life
    );
}

fn carry_native_key_item(owner: &mut Owner, name: &str) -> String {
    let template = mir2_game_data::crystal_item_by_name(name).unwrap();
    let mut save = owner.runtime.inner.active_character_checkpoint().unwrap();
    let mut item: serde_json::Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    let key = format!("crystal-item-{}", template.item_index);
    item["key"] = serde_json::json!(key);
    item["name"] = serde_json::json!(name);
    item["icon"] = serde_json::json!(template.image);
    item["shape"] = serde_json::json!(template.shape);
    item["unique_id"] = serde_json::json!(90377);
    item["slot"] = serde_json::json!(0);
    item["quantity"] = serde_json::json!(1);
    save.inventory_items_json = vec![item.to_string()];
    owner
        .runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    owner.runtime.sync_zone_snapshot();
    owner.flush();
    key
}

#[test]
fn owner_health_routing_authenticated_key_sun_potion_publishes_exact_pools_once() {
    let mut owner = Owner::with_config(256, GatewayConfig::default().with_platinum_176_profile());
    let key = carry_native_key_item(&mut owner, "SunPotion");
    let mut state = owner.shared.lock().unwrap();
    let out = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 10, 18, 5, false);
    state.dispatch_zone_outbounds(out, Some(&owner.key));
    drop(state);
    owner.flush();
    owner
        .runtime
        .execute(WorldCommand::UseItem { key })
        .unwrap();
    assert_eq!(pools(&owner.flush()), vec![(18, 5), (18, 14)]);
    assert_eq!(
        owner
            .shared
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&owner.session),
        Some((18, 18, 14))
    );
    assert!(owner
        .runtime
        .inner
        .world_snapshot()
        .inventory_items
        .iter()
        .all(|i| i.name != "SunPotion"));
}

#[test]
fn owner_health_routing_authenticated_key_town_scroll_commits_same_map_relocation() {
    let mut owner = Owner::with_config(256, GatewayConfig::default().with_platinum_176_profile());
    let key = carry_native_key_item(&mut owner, "TownTeleport");
    let initial = owner.runtime.inner.active_character_checkpoint().unwrap();
    let bind = initial.bind_point.clone().unwrap();
    owner
        .runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: Point {
                x: bind.position.x + 2,
                y: bind.position.y + 2,
            },
            direction: MirDirection::Right,
            hp: None,
            mp: None,
        })
        .unwrap();
    let packets = owner
        .runtime
        .execute(WorldCommand::UseItem { key })
        .unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::UserLocation { .. })),
        "{packets:?}"
    );
    let transform = owner
        .shared
        .lock()
        .unwrap()
        .zone_manager
        .player_transform(&owner.session)
        .unwrap();
    assert_eq!(transform.0, bind.position);
    assert_eq!(
        owner
            .runtime
            .inner
            .active_character_checkpoint()
            .unwrap()
            .position,
        bind.position
    );
    assert!(owner
        .runtime
        .inner
        .world_snapshot()
        .inventory_items
        .iter()
        .all(|i| i.name != "TownTeleport"));
}

fn level_award(owner: &mut Owner) -> Vec<ZoneMonsterKillAward> {
    cross_map_drop_lifecycle_tests::issue_prepared_award(
        &mut owner.runtime,
        ZoneMonsterKillAward {
            source_receipt_key: None,
            mentor_bank: None,
            experience_selection: None,
            monster_object_id: 94401,
            killed_at_ms: 2000,
            monster_name: "Field Wasp".into(),
            experience: 100,
            drops: Vec::new(),
            boss_audit: None,
        },
    )
}

#[derive(Debug)]
struct LevelReceiptInterleave {
    delegate: InProcessAccountInventoryService,
    shared: Arc<Mutex<SharedInProcessZoneState>>,
    key: ZonePresenceKey,
    session: SessionId,
    revive: bool,
}

impl SharedAccountInventoryService for LevelReceiptInterleave {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        let receipt = self.delegate.commit(runtime, envelope);
        if receipt.committed
            && receipt
                .packets
                .iter()
                .any(|p| matches!(p, ServerPacket::LevelChanged { .. }))
        {
            // Real confirmed Source receipt, prepared shared-life interleaving.
            // This does not claim a natural simultaneous monster hit.
            let mut state = self.shared.lock().unwrap();
            let (_, max_hp, mp) = state.zone_manager.player_vitals(&self.session).unwrap();
            let out = state
                .zone_manager
                .commit_owner_vitals(&self.session, 0, max_hp, mp, true);
            state.dispatch_zone_outbounds(out, Some(&self.key));
            if self.revive {
                let out =
                    state
                        .zone_manager
                        .commit_owner_vitals(&self.session, 5, max_hp, mp, false);
                state.dispatch_zone_outbounds(out, Some(&self.key));
            }
        }
        receipt
    }
}

#[test]
fn owner_health_routing_confirmed_level_keeps_ceiling_but_cannot_replay_old_life_refill() {
    for revive in [false, true] {
        let mut owner = Owner::new(256);
        let awards = level_award(&mut owner);
        owner.flush();
        owner.runtime.account_inventory_service = Arc::new(LevelReceiptInterleave {
            delegate: InProcessAccountInventoryService::default(),
            shared: owner.shared.clone(),
            key: owner.key.clone(),
            session: owner.session.clone(),
            revive,
        });
        let packets = owner.runtime.apply_zone_monster_kill_awards(awards);
        assert!(packets
            .iter()
            .any(|p| matches!(p, ServerPacket::LevelChanged { level: 2, .. })));
        let state = owner.shared.lock().unwrap();
        assert_eq!(
            state
                .zone_manager
                .player_vitals(&owner.session)
                .map(|p| (p.0, p.1, p.2)),
            Some((if revive { 5 } else { 0 }, 24, 14))
        );
        assert_eq!(
            state.zone_manager.player_is_dead(&owner.session),
            Some(!revive)
        );
        let life = state
            .zone_manager
            .owner_health_cursor(&owner.session)
            .unwrap()
            .life_generation;
        drop(state);
        assert!(pools(&owner.flush())
            .iter()
            .all(|(hp, mp)| *hp != 24 && *mp == 14));
        assert_eq!(
            owner
                .shared
                .lock()
                .unwrap()
                .zone_manager
                .owner_health_cursor(&owner.session)
                .unwrap()
                .life_generation,
            life
        );
    }
}

#[test]
fn owner_health_routing_live_queued_level_is_saved_in_actual_fenced_teardown() {
    let mut owner = Owner::new(256);
    let awards = level_award(&mut owner);
    owner
        .shared
        .lock()
        .unwrap()
        .prepend_zone_monster_kill_awards(owner.key.clone(), awards);
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
    let prepared = owner
        .runtime
        .prepare_teardown_checkpoint(&lease)
        .unwrap()
        .unwrap();
    let saved = prepared.checkpoint();
    assert_eq!(
        (
            saved.character.level,
            saved.hp,
            saved.max_hp,
            saved.mp,
            saved.max_mp
        ),
        (2, 24, 24, 18, 18)
    );
    assert!(!owner.registration.is_current());
}

#[test]
fn owner_health_routing_personal_delta_cannot_cross_replaced_teardown_fence() {
    let mut owner = prepared_normal_potion();
    owner
        .shared
        .lock()
        .unwrap()
        .begin_teardown_fence(&owner.key)
        .unwrap();
    let basis = owner
        .runtime
        .prepare_teardown_personal_vital_command(&owner.key)
        .unwrap()
        .unwrap();
    let mut packets = owner
        .runtime
        .inner
        .try_tick_shared_zone_personal_state()
        .unwrap();
    let mut state = owner.shared.lock().unwrap();
    state.release_teardown_fence(&owner.key);
    state.begin_teardown_fence(&owner.key).unwrap();
    drop(state);
    owner
        .runtime
        .commit_shared_personal_vital_command(basis, &mut packets)
        .unwrap();
    assert_eq!(
        owner
            .shared
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&owner.session),
        Some((10, 18, 14))
    );
    assert_eq!(
        owner.runtime.inner.local_player_vitals_snapshot().player_hp,
        Some(10)
    );
}

#[test]
fn owner_health_routing_actual_town_revive_tail_preserves_new_life_damage() {
    let mut owner = Owner::new(256);
    owner
        .runtime
        .execute(WorldCommand::ApplyHandoffTransform {
            position: owner
                .runtime
                .inner
                .active_character_checkpoint()
                .unwrap()
                .position,
            direction: MirDirection::Right,
            hp: Some(0),
            mp: None,
        })
        .unwrap();
    let revived = owner
        .runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .unwrap();
    assert!(revived.iter().any(|p| matches!(p, ServerPacket::Revived)));
    owner.flush();
    let mut state = owner.shared.lock().unwrap();
    let life = state
        .zone_manager
        .owner_health_cursor(&owner.session)
        .unwrap()
        .life_generation;
    let (_, max_hp, mp) = state.zone_manager.player_vitals(&owner.session).unwrap();
    let out = state
        .zone_manager
        .commit_owner_vitals(&owner.session, 7, max_hp, mp, false);
    state.dispatch_zone_outbounds(out, Some(&owner.key));
    drop(state);
    // Prepared interleaving at the actual metadata-tail method, after ordinary
    // TownRevive committed its new life. No natural concurrent hit is claimed.
    owner.runtime.sync_zone_snapshot_preserving_owner_vitals();
    assert_eq!(
        owner
            .shared
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&owner.session)
            .unwrap()
            .0,
        7
    );
    assert_eq!(
        owner
            .shared
            .lock()
            .unwrap()
            .zone_manager
            .owner_health_cursor(&owner.session)
            .unwrap()
            .life_generation,
        life
    );
    assert_eq!(pools(&owner.flush()), vec![(7, mp)]);
}

#[test]
fn owner_health_routing_closed_at_fifo_cap_preserves_normal_death_for_source_drain() {
    let mut owner = Owner::new(1);
    owner.commit(10, 20, false);
    owner.flush();
    owner.commit(9, 20, false); // Occupy the real stamped sender.
    for index in 0..MAX_PENDING_ZONE_PACKETS_PER_PLAYER {
        owner.shared.lock().unwrap().queue_zone_packets(
            owner.key.clone(),
            vec![ServerPacket::DamageIndicator {
                object_id: 900_000 + index as u32,
                damage: 1,
                damage_type: 0,
            }],
        );
    }
    let (_, empty_receiver) = tokio::sync::mpsc::channel(1);
    drop(std::mem::replace(&mut owner.rx, empty_receiver));
    let mut state = owner.shared.lock().unwrap();
    state.queue_zone_packets(
        owner.key.clone(),
        vec![ServerPacket::Death {
            location: Point { x: 1, y: 1 },
            direction: MirDirection::Right,
        }],
    );
    assert!(!state.live_zone_outbounds.contains_key(&owner.key));
    let packets = state.take_pending_zone_packets(&owner.key);
    assert_eq!(packets.len(), MAX_PENDING_ZONE_PACKETS_PER_PLAYER + 1);
    assert!(matches!(packets.last(), Some(ServerPacket::Death { .. })));
    assert!(state.take_pending_zone_packets(&owner.key).is_empty());
    assert!(packets
        .iter()
        .all(|p| !matches!(p, ServerPacket::HealthChanged { .. })));
}

fn prepared_mentor_pair(
    owner: &mut Owner,
) -> (GatewayConfig, mir2_simulation::Stage5FriendIdentity) {
    let config = owner.runtime.inner.shared_mentor_config().unwrap();
    let pupil = mir2_simulation::Stage5FriendIdentity {
        account_id: owner.key.account_id.clone(),
        character_index: owner.key.character_index,
    };
    let teacher = mir2_simulation::Stage5FriendIdentity {
        account_id: "health-mentor-teacher".into(),
        character_index: 9,
    };
    let mut character = config.default_character.clone();
    character.index = teacher.character_index;
    character.name = "HealthTeacher".into();
    character.level = 11;
    config.account_store.lock().unwrap().accounts.insert(
        teacher.account_id.clone(),
        mir2_simulation::AccountRecord::new(character),
    );
    config
        .commit_shared_mentor_mutation(
            &teacher,
            mir2_simulation::SharedMentorMutation::TogglePermission,
            SharedInProcessZoneSessionRuntime::zone_now_ms(),
        )
        .unwrap();
    config
        .commit_shared_mentor_mutation(
            &teacher,
            mir2_simulation::SharedMentorMutation::Accept { student: pupil },
            SharedInProcessZoneSessionRuntime::zone_now_ms(),
        )
        .unwrap();
    owner.runtime.ranking_zones.lock().unwrap().insert(
        ZoneId::new("test-shared-zone"),
        crate::routing::SharedInProcessZoneResources {
            zone_state: owner.shared.clone(),
            movement_sender: owner.runtime.movement_ingress.movement_sender.clone(),
            tick_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            autonomous_ticks_enabled: Arc::new(AtomicBool::new(false)),
        },
    );
    (config, teacher)
}

fn prepared_reward_credit(
    config: &GatewayConfig,
    actor: &mir2_simulation::Stage5FriendIdentity,
    amount: u64,
) {
    // Only test preparation queues durable credit. Consumption below uses the
    // actual Source rollback/persistence reducer and shared recovery bridge.
    let mut store = config.account_store.lock().unwrap();
    let save = store
        .accounts
        .get_mut(&actor.account_id)
        .unwrap()
        .saves
        .get_mut(&actor.character_index)
        .unwrap();
    let mut systems: mir2_simulation::Stage5SystemsState =
        serde_json::from_str(save.stage5_systems_json.as_ref().unwrap()).unwrap();
    systems.mentor.ledger.leveling_credit = amount;
    save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
}

#[test]
fn owner_health_routing_mentor_source_level_graduates_immediately_and_during_fenced_teardown() {
    for teardown in [false, true] {
        let mut owner = Owner::new(256);
        let (config, teacher) = prepared_mentor_pair(&mut owner);
        let pupil = mir2_simulation::Stage5FriendIdentity {
            account_id: owner.key.account_id.clone(),
            character_index: owner.key.character_index,
        };
        let amount = owner
            .runtime
            .inner
            .active_character_checkpoint()
            .unwrap()
            .max_experience;
        prepared_reward_credit(&config, &pupil, amount as u64);
        assert_eq!(
            owner.shared.lock().unwrap().players[&owner.key]
                .entity
                .level,
            Some(1)
        );
        if teardown {
            let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
            let saved = owner
                .runtime
                .prepare_teardown_checkpoint(&lease)
                .unwrap()
                .unwrap();
            assert_eq!(saved.checkpoint().character.level, 2);
        } else {
            let packets = owner
                .runtime
                .reconcile_mentor_accounting(false, false)
                .unwrap();
            assert!(packets
                .iter()
                .any(|p| matches!(p, ServerPacket::LevelChanged { level: 2, .. })));
            assert_eq!(
                owner.shared.lock().unwrap().players[&owner.key]
                    .entity
                    .level,
                Some(1),
                "Source level is confirmed before viewport metadata"
            );
        }
        assert!(config
            .shared_mentor_profile_for(&pupil)
            .unwrap()
            .mentor
            .partner_identity
            .is_none());
        assert!(config
            .shared_mentor_profile_for(&teacher)
            .unwrap()
            .mentor
            .partner_identity
            .is_none());
        assert_eq!(
            config
                .shared_mentor_profile_for(&pupil)
                .unwrap()
                .mentor
                .cooldown_until_ms,
            0
        );
        assert_eq!(
            config
                .shared_mentor_profile_for(&pupil)
                .unwrap()
                .mentor
                .ledger
                .leveling_applied,
            amount as u64
        );
    }
}

#[test]
fn owner_health_routing_mentor_peer_uses_confirmed_source_level_before_viewport_refresh() {
    let mut owner = Owner::new(256);
    let (config, teacher_id) = prepared_mentor_pair(&mut owner);
    let mut teacher = shared_session_runtime(owner.shared.clone());
    teacher.ranking_zones = owner.runtime.ranking_zones.clone();
    teacher.ranking_replica_zones = owner.runtime.ranking_replica_zones.clone();
    teacher.shared_social_generation = owner.runtime.shared_social_generation.clone();
    teacher
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: teacher_id.account_id.clone(),
            password: "demo".into(),
        }))
        .unwrap();
    teacher
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: teacher_id.character_index,
        }))
        .unwrap();
    let teacher_key = teacher.current_presence_key().unwrap();
    let initial = owner.runtime.execute(WorldCommand::Tick).unwrap();
    assert!(initial.iter().any(|p| matches!(p, ServerPacket::MentorUpdate {
        level: 11, online: true, ..
    })));
    let amount = teacher
        .inner
        .active_character_checkpoint()
        .unwrap()
        .max_experience as u64;
    prepared_reward_credit(&config, &teacher_id, amount);
    let packets = teacher.reconcile_mentor_accounting(false, false).unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LevelChanged { level: 12, .. })));
    assert_eq!(
        owner.shared.lock().unwrap().players[&teacher_key]
            .entity
            .level,
        Some(11)
    );
    let pupil = mir2_simulation::Stage5FriendIdentity {
        account_id: owner.key.account_id.clone(),
        character_index: owner.key.character_index,
    };
    let amount = owner
        .runtime
        .inner
        .active_character_checkpoint()
        .unwrap()
        .max_experience as u64;
    prepared_reward_credit(&config, &pupil, amount);
    owner
        .runtime
        .reconcile_mentor_accounting(false, false)
        .unwrap();
    assert_eq!(
        config
            .shared_mentor_profile_for(&pupil)
            .unwrap()
            .mentor
            .partner_identity,
        Some(teacher_id)
    );
    let updated = owner.runtime.execute(WorldCommand::Tick).unwrap();
    assert!(updated.iter().any(|p| matches!(p,
        ServerPacket::MentorUpdate {
            level: 12,
            online: true,
            ..
        }
    )), "ordinary Tick must notify the confirmed peer level: {updated:?}");
}
