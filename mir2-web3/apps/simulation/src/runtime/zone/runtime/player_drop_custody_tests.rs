use super::*;
use crate::player_drop::PlayerDropOperationBinding;
use crate::runtime::zone::{ZoneBounds, ZoneJoin};
use mir2_protocol::{MirClass, MirGender};

pub(super) fn intent_for(
    zone: &ZoneKey,
    nonce: &str,
    position: Point,
    amount: u32,
) -> PlayerDropWorldIntent {
    let binding = PlayerDropOperationBinding::new(
        nonce.into(),
        "source-rpc".into(),
        "source-account".into(),
        0,
        crate::zone_id_for_key(zone),
        "owner-a".into(),
        1,
        7,
    )
    .unwrap();
    PlayerDropWorldIntent::new(
        binding,
        zone.clone(),
        position,
        1_000,
        10_000,
        PlayerDropOutcome::Gold { amount },
    )
    .unwrap()
}
fn proof(intent: PlayerDropWorldIntent, state: PlayerDropEffectState) -> PlayerDropCommittedEffect {
    PlayerDropCommittedEffect::for_zone_test(intent, state)
}
fn join(zone: &mut ZoneRuntime, name: &str, object_id: u32, position: Point) -> SessionId {
    let session = SessionId::new(name);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session.clone(),
        account_id: name.into(),
        character_index: 0,
        object_id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 10,
        hp: 100,
        max_hp: 100,
        mp: 20,
        map_file_name: zone.key.map_file_name.clone(),
        position,
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    session
}
fn claim(
    zone: &mut ZoneRuntime,
    session: &SessionId,
    object_id: u32,
    now_ms: u64,
) -> GroundDropClaimTicket {
    let target = {
        let d = &zone.ground_drops[&object_id].drop;
        Point { x: d.x, y: d.y }
    };
    zone.claim_ground_drop(session, Some(object_id), &target, &[], now_ms)
        .into_iter()
        .find_map(|out| match out {
            ZoneOutbound::GroundDropClaimedWithTicket { ticket, .. } => Some(ticket),
            _ => None,
        })
        .unwrap()
}

#[test]
fn player_drop_reservation_enforces_occupancy_item_and_gold_ranges_and_five_slot_capacity() {
    let key = ZoneKey::for_map("player-drop-placement");
    let collision = ZoneCollision::unbounded()
        .with_bounds(ZoneBounds::new(0, 5, 0, 0))
        .with_blocked_cells([
            Point { x: 1, y: 0 },
            Point { x: 2, y: 0 },
            Point { x: 3, y: 0 },
            Point { x: 4, y: 0 },
        ]);
    let mut zone = ZoneRuntime::new_with_collision(key, collision);
    let session = join(&mut zone, "source", 101, Point { x: 0, y: 0 });
    assert!(zone
        .reserve_player_drop_placement(
            &session,
            "item-no-room",
            PlayerDropPlacementKind::Item,
            1_000
        )
        .is_err());
    for n in 0..5 {
        let slot = zone
            .reserve_player_drop_placement(
                &session,
                &format!("gold-{n}"),
                PlayerDropPlacementKind::Gold,
                1_000,
            )
            .unwrap();
        assert_eq!(slot.position(), &Point { x: 5, y: 0 });
    }
    assert!(zone
        .reserve_player_drop_placement(&session, "gold-sixth", PlayerDropPlacementKind::Gold, 1_000)
        .is_err());
    assert!(
        zone.native_ground_drop_position(&Point { x: 5, y: 0 })
            .is_none(),
        "monster drops share reserved five-drop capacity"
    );
    let slot = zone.player_drop_reservations["gold-0"].clone();
    assert!(zone.release_player_drop_placement(&slot));
    assert!(zone
        .reserve_player_drop_placement(&session, "gold-sixth", PlayerDropPlacementKind::Gold, 1_001)
        .is_ok());
    assert!(zone
        .reserve_player_drop_placement(&session, "gold-1", PlayerDropPlacementKind::Item, 1_001)
        .is_err());
}

#[test]
fn player_drop_reservation_replays_exactly_and_frozen_location_mismatch_rejects_without_minting() {
    let key = ZoneKey::for_map("player-drop-frozen");
    let mut zone = ZoneRuntime::new_with_collision(key.clone(), ZoneCollision::unbounded());
    let session = join(&mut zone, "source", 101, Point { x: 10, y: 10 });
    let mut intent = intent_for(&key, "frozen", Point { x: 11, y: 10 }, 25);
    let slot = zone
        .reserve_player_drop_placement(
            &session,
            intent.world_effect_id(),
            PlayerDropPlacementKind::Gold,
            1_000,
        )
        .unwrap();
    assert_ne!(
        slot.position(),
        &Point { x: 10, y: 10 },
        "occupied source cell is excluded"
    );
    assert_eq!(
        zone.reserve_player_drop_placement(
            &session,
            intent.world_effect_id(),
            PlayerDropPlacementKind::Gold,
            1_500
        )
        .unwrap(),
        slot
    );
    intent = intent_for(&key, "frozen", Point { x: 999, y: 999 }, 25);
    assert!(zone
        .apply_committed_player_drop(&proof(intent, PlayerDropEffectState::Live), 2_000)
        .is_err());
    assert!(zone.ground_drops.is_empty());
}

#[test]
fn player_drop_committed_apply_needs_no_session_replays_once_and_rejects_changed_payload() {
    let key = ZoneKey::for_map("player-drop-cold");
    let mut zone = ZoneRuntime::new(key.clone());
    let intent = intent_for(&key, "committed", Point { x: 10, y: 10 }, 25);
    let evidence = proof(intent.clone(), PlayerDropEffectState::Live);
    let first = zone.apply_committed_player_drop(&evidence, 2_000).unwrap();
    let id = first.object_id.unwrap();
    assert!(!first.replay);
    assert!(
        zone.apply_committed_player_drop(&evidence, 2_001)
            .unwrap()
            .replay
    );
    assert_eq!(zone.ground_drop_count(), 1);
    assert!(zone
        .query_player_drop_effect(intent.world_effect_id(), intent.intent_digest())
        .unwrap()
        .is_some());
    assert!(zone
        .query_player_drop_effect(intent.world_effect_id(), "mismatched-intent")
        .is_err());
    assert_eq!(zone.objects[&id].expires_at_ms, Some(10_000));
    assert!(
        matches!(&zone.objects[&id].packet, ServerPacket::ObjectGold { info } if info.gold == 25)
    );
    let changed = intent_for(&key, "committed", Point { x: 10, y: 10 }, 26);
    assert!(zone
        .apply_committed_player_drop(&proof(changed, PlayerDropEffectState::Live), 2_002)
        .is_err());
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    restored
        .reconcile_committed_player_drops(&[evidence.clone()], 3_000)
        .unwrap();
    let new_id = match restored
        .player_drop_effect_state(intent.world_effect_id())
        .unwrap()
    {
        PlayerDropCustodyState::Live { object_id, .. } => object_id,
        _ => panic!("live"),
    };
    assert_ne!(new_id, id);
    assert_eq!(restored.objects[&new_id].expires_at_ms, Some(10_000));
    assert!(restored.players.is_empty());
    restored
        .reconcile_committed_player_drops(&[evidence], 10_000)
        .unwrap();
    assert_eq!(restored.ground_drop_count(), 0);
    assert_eq!(
        restored.player_drop_effect_state(intent.world_effect_id()),
        Some(PlayerDropCustodyState::Expired)
    );
}

#[test]
fn player_drop_recovery_renews_presentation_but_pickup_key_is_effect_based_and_old_claim_is_revoked(
) {
    let key = ZoneKey::for_map("player-drop-pickup-identity");
    let mut zone = ZoneRuntime::new_with_collision(key.clone(), ZoneCollision::unbounded());
    let intent = intent_for(&key, "pickup", Point { x: 10, y: 10 }, 25);
    let evidence = proof(intent.clone(), PlayerDropEffectState::Live);
    let first = zone
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let alice = join(&mut zone, "alice", 101, Point { x: 9, y: 10 });
    let old = claim(&mut zone, &alice, first, 2_001);
    assert_eq!(
        old.idempotency_key,
        format!("player-drop-pickup:{}", intent.world_effect_id())
    );
    assert!(zone.has_pending_ground_drop_claim_ticket(&alice, &old));
    zone.reconcile_committed_player_drops(&[evidence.clone()], 3_000)
        .unwrap();
    assert!(!zone.has_pending_ground_drop_claim_ticket(&alice, &old));
    assert!(!zone.has_detached_ground_drop_claim_ticket(&old));
    assert!(zone
        .cancel_ground_drop_claim(&alice, &old, 3_001)
        .is_empty());
    let second = *zone.ground_drops.keys().next().unwrap();
    let bob = join(&mut zone, "bob", 102, Point { x: 11, y: 10 });
    let fresh = claim(&mut zone, &bob, second, 3_001);
    assert_ne!(old.object_id, fresh.object_id);
    assert_ne!(old.drop_generation, fresh.drop_generation);
    assert_ne!(old.payload_digest, fresh.payload_digest);
    assert_eq!(old.idempotency_key, fresh.idempotency_key);
    assert_eq!(old.player_drop_effect, fresh.player_drop_effect);
    zone.commit_ground_drop_claim(&bob, &fresh);
    assert_eq!(
        zone.player_drop_effect_state(intent.world_effect_id()),
        Some(PlayerDropCustodyState::Consumed)
    );
    assert_eq!(
        zone.apply_committed_player_drop(&evidence, 4_000)
            .unwrap()
            .object_id,
        None,
        "late Live receipt cannot revive consumed effect"
    );
    assert_eq!(zone.ground_drop_count(), 0);
}

#[test]
fn player_drop_held_claim_expiry_and_terminal_reconciliation_prevent_snapshot_resurrection() {
    let key = ZoneKey::for_map("player-drop-terminal");
    let mut zone = ZoneRuntime::new_with_collision(key.clone(), ZoneCollision::unbounded());
    let intent = intent_for(&key, "terminal", Point { x: 10, y: 10 }, 25);
    let id = zone
        .apply_committed_player_drop(&proof(intent.clone(), PlayerDropEffectState::Live), 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let alice = join(&mut zone, "alice", 101, Point { x: 9, y: 10 });
    let old = claim(&mut zone, &alice, id, 2_001);
    assert!(zone.detach_ground_drop_claim(&alice, &old));
    zone.expire_player_drop_effects(10_000);
    assert!(!zone.has_detached_ground_drop_claim_ticket(&old));
    assert!(zone
        .restore_detached_ground_drop_claim(&old, 10_001)
        .is_none());
    zone.reconcile_committed_player_drops(
        &[proof(intent.clone(), PlayerDropEffectState::Consumed)],
        10_001,
    )
    .unwrap();
    assert_eq!(
        zone.player_drop_effect_state(intent.world_effect_id()),
        Some(PlayerDropCustodyState::Consumed)
    );
    assert!(zone.ground_drops.is_empty());
    assert!(zone.claimed_ground_drops.is_empty());
    assert!(zone.detached_ground_drop_custody.is_empty());
    zone.reconcile_committed_player_drops(&[], 11_000).unwrap();
    assert!(zone.player_drop_effects.is_empty());
}

#[test]
fn player_drop_reservation_excludes_transfer_sources() {
    let key = ZoneKey::for_map("player-drop-transfer-source");
    let collision = ZoneCollision::unbounded()
        .with_bounds(ZoneBounds::new(0, 1, 0, 0))
        .with_transfer_source_cells_for_test([Point { x: 1, y: 0 }]);
    let mut zone = ZoneRuntime::new_with_collision(key, collision);
    let session = join(&mut zone, "source", 101, Point { x: 0, y: 0 });
    assert!(zone
        .reserve_player_drop_placement(
            &session,
            "transfer-only",
            PlayerDropPlacementKind::Item,
            1_000
        )
        .is_err());
}

#[test]
fn player_drop_fresh_host_counter_reuse_cannot_reuse_old_claimant_incarnation() {
    let key = ZoneKey::for_map("player-drop-fresh-host-aba");
    let intent = intent_for(&key, "fresh-host", Point { x: 10, y: 10 }, 25);
    let evidence = proof(intent, PlayerDropEffectState::Live);
    let mut old_host = ZoneRuntime::new_with_collision(key.clone(), ZoneCollision::unbounded());
    let old_id = old_host
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let old_session = join(&mut old_host, "same-session", 101, Point { x: 9, y: 10 });
    let old_ticket = claim(&mut old_host, &old_session, old_id, 2_001);
    let mut new_host = ZoneRuntime::new_with_collision(key, ZoneCollision::unbounded());
    let new_id = new_host
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let new_session = join(&mut new_host, "same-session", 101, Point { x: 9, y: 10 });
    let new_ticket = claim(&mut new_host, &new_session, new_id, 2_001);
    assert_eq!(old_ticket.object_id, new_ticket.object_id);
    assert_eq!(old_ticket.drop_generation, new_ticket.drop_generation);
    assert_eq!(old_ticket.claim_id, new_ticket.claim_id);
    assert_eq!(old_ticket.idempotency_key, new_ticket.idempotency_key);
    assert_ne!(
        old_ticket.player_drop_claimant,
        new_ticket.player_drop_claimant
    );
    assert!(!new_host.has_pending_ground_drop_claim_ticket(&new_session, &old_ticket));
    assert!(new_host.detach_ground_drop_claim(&new_session, &new_ticket));
    assert!(!new_host.has_detached_ground_drop_claim_ticket(&old_ticket));
    assert!(new_host.has_detached_ground_drop_claim_ticket(&new_ticket));
}

#[test]
fn player_drop_exact_item_and_embedded_instances_recover_without_attribute_loss() {
    use crate::config::GroundDropItemPayload;
    use mir2_protocol::{UserItem, UserItemExpireInfo, UserItemStat};
    let key = ZoneKey::for_map("player-drop-exact-item");
    let mut item = UserItem {
        unique_id: 8_888,
        item_index: 404,
        current_dura: 4_321,
        max_dura: 5_432,
        count: 1,
        soul_bound_id: 17,
        identified: false,
        cursed: true,
        slots: Vec::new(),
        gem_count: 1,
        added_stats: vec![UserItemStat { stat: 17, value: 9 }],
        awake_type: 2,
        awake_values: vec![3, 4],
        refined_value: 5,
        refine_added: 6,
        refine_success_chance: 77,
        wedding_ring: 23,
        expire_info: Some(UserItemExpireInfo {
            expiry_binary_datetime: 123_456,
        }),
        rental_information: None,
        is_shop_item: true,
        sealed_info: None,
        gm_made: true,
    };
    let mut slot = item.clone();
    slot.unique_id = 8_889;
    slot.gem_count = 0;
    item.slots = vec![Some(slot), None];
    let exact = GroundDropItemPayload {
        item,
        uid_assigned: true,
    };
    let loot = GroundDropLootSnapshot::InventoryItem {
        key: "crystal-item-404".into(),
        name: "CopperRing".into(),
        description: "exact item".into(),
        weight: 1,
        durability_current: Some(4_321),
        durability_max: Some(5_432),
        added_attack: 3,
        added_defence: 7,
        added_stats: vec![UserItemStat { stat: 17, value: 9 }],
        cursed: true,
        socket_slots: 2,
        show_group_pickup: false,
        exact_item: Some(exact),
    };
    let binding = PlayerDropOperationBinding::new(
        "exact".into(),
        "source-rpc".into(),
        "source-account".into(),
        0,
        crate::zone_id_for_key(&key),
        "owner-a".into(),
        1,
        7,
    )
    .unwrap();
    let intent = PlayerDropWorldIntent::new(
        binding,
        key.clone(),
        Point { x: 10, y: 10 },
        1_000,
        10_000,
        PlayerDropOutcome::Item {
            quantity: 1,
            loot: loot.clone(),
        },
    )
    .unwrap();
    let evidence = proof(intent, PlayerDropEffectState::Live);
    let mut zone = ZoneRuntime::new(key);
    let id = zone
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    assert_eq!(zone.ground_drops[&id].drop.loot, loot);
    assert!(
        matches!(&zone.objects[&id].packet, ServerPacket::ObjectItem { info } if info.object_id == id && info.name == "CopperRing")
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    restored
        .reconcile_committed_player_drops(&[evidence], 3_000)
        .unwrap();
    assert_eq!(
        restored.ground_drops.values().next().unwrap().drop.loot,
        loot
    );
}

#[test]
fn player_drop_replica_mode_is_explicit_and_checkpoint_cannot_grant_it() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("player-drop-replica-mode"));
    assert!(!zone.is_player_drop_replica_mode());
    let root = zone.canonical_state_root().unwrap();
    zone.set_player_drop_replica_mode(true);
    assert!(zone.is_player_drop_replica_mode());
    assert!(zone.transaction_fork().is_player_drop_replica_mode());
    assert_eq!(
        zone.canonical_state_root().unwrap(),
        root,
        "process mode is not world state"
    );
    let restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(
        !restored.is_player_drop_replica_mode(),
        "checkpoint cannot grant privileged journal replay"
    );
}

#[test]
fn player_drop_replica_installs_frozen_placement_without_selecting_a_new_cell() {
    let key = ZoneKey::for_map("player-drop-frozen-replica");
    let mut source = ZoneRuntime::new_with_collision(key, ZoneCollision::unbounded());
    let session = join(&mut source, "source", 101, Point { x: 10, y: 10 });
    let mut replica = source.transaction_fork();
    let slot = source
        .reserve_player_drop_placement(
            &session,
            "frozen-replica",
            PlayerDropPlacementKind::Gold,
            1_000,
        )
        .unwrap();
    assert!(source
        .install_frozen_player_drop_reservation(&slot)
        .is_err());
    replica.set_player_drop_replica_mode(true);
    replica
        .install_frozen_player_drop_reservation(&slot)
        .unwrap();
    assert_eq!(
        replica.canonical_state_root().unwrap(),
        source.canonical_state_root().unwrap()
    );
    assert!(replica
        .install_frozen_player_drop_reservation(&slot)
        .is_ok());
    let mut changed = slot.clone();
    changed.position.x += 1;
    assert!(replica
        .install_frozen_player_drop_reservation(&changed)
        .is_err());
}

#[test]
fn player_drop_cancel_at_absolute_expiry_boundary_cannot_restore_ground_object() {
    let key = ZoneKey::for_map("player-drop-expiry-boundary");
    let intent = intent_for(&key, "expiry-boundary", Point { x: 10, y: 10 }, 25);
    let evidence = proof(intent.clone(), PlayerDropEffectState::Live);
    let mut zone = ZoneRuntime::new_with_collision(key, ZoneCollision::unbounded());
    let id = zone
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let session = join(&mut zone, "claimant", 101, Point { x: 9, y: 10 });
    let ticket = claim(&mut zone, &session, id, 9_999);
    zone.cancel_ground_drop_claim(&session, &ticket, 10_000);
    assert!(zone.ground_drops.is_empty());
    assert!(zone.claimed_ground_drops.is_empty());
    assert!(zone.objects.get(&id).is_none());
    assert_eq!(
        zone.player_drop_effect_state(intent.world_effect_id()),
        Some(PlayerDropCustodyState::Expired)
    );
}

#[test]
fn player_drop_cold_retained_player_has_no_claim_authority_until_fresh_join() {
    let key = ZoneKey::for_map("player-drop-cold-claim-authority");
    let intent = intent_for(&key, "cold-claim", Point { x: 10, y: 10 }, 25);
    let evidence = proof(intent, PlayerDropEffectState::Live);
    let mut zone = ZoneRuntime::new(key);
    let id = zone
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let session = join(&mut zone, "claimant", 101, Point { x: 9, y: 10 });
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(restored
        .claim_ground_drop(&session, Some(id), &Point { x: 10, y: 10 }, &[], 2_001)
        .is_empty());
    assert!(restored.ground_drops.contains_key(&id));
    assert!(restored.claimed_ground_drops.is_empty());
    join(&mut restored, "claimant", 101, Point { x: 9, y: 10 });
    let ticket = claim(&mut restored, &session, id, 2_002);
    assert!(ticket.player_drop_claimant.is_some());
}

#[test]
fn player_drop_reservation_rejection_is_transient_and_unresolved_slots_block_checkpoint_export() {
    use crate::runtime::zone::{LogicalWorldId, ZoneManager};
    let key = ZoneKey::for_map("player-drop-transient-reservation");
    let mut manager = ZoneManager::new();
    manager
        .bind_logical_world(
            LogicalWorldId::legacy_execution_domain("transient-player-drop").unwrap(),
        )
        .unwrap();
    assert!(manager.install_empty_zone(ZoneRuntime::new_with_collision(
        key.clone(),
        ZoneCollision::unbounded()
    )));
    let session = SessionId::new("source");
    manager.handle_for_key(
        key.clone(),
        ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "source-account".into(),
            character_index: 0,
            object_id: 101,
            name: "Source".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 10,
            hp: 100,
            max_hp: 100,
            mp: 20,
            map_file_name: key.map_file_name.clone(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Down,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }),
    );
    let before = manager.checkpoint_bytes().unwrap();
    let before_root = manager.zone(&key).unwrap().canonical_state_root().unwrap();
    let before_admissions = manager.admitted_action_count();
    let mut replica = manager.fork_for_deterministic_reference();
    replica.set_player_drop_replica_mode_for_key(&key, true);
    let slot = manager
        .reserve_player_drop_placement_for_key(
            &key,
            &session,
            "transient-slot",
            PlayerDropPlacementKind::Gold,
            1_000,
        )
        .unwrap();
    assert_eq!(manager.admitted_action_count(), before_admissions);
    assert_eq!(
        manager.zone(&key).unwrap().canonical_state_root().unwrap(),
        before_root,
        "a private capacity hold is not committed business state"
    );
    assert!(manager
        .checkpoint_bytes()
        .unwrap_err()
        .contains("unresolved player-drop placement reservations"));
    replica
        .install_player_drop_replica_reservation_for_key(&key, &slot)
        .unwrap();
    assert_eq!(
        replica.admitted_action_count(),
        before_admissions,
        "authenticated frozen slot replay is also transient"
    );
    assert_eq!(
        replica.zone(&key).unwrap().canonical_state_root().unwrap(),
        before_root
    );
    assert!(replica.checkpoint_bytes().is_err());
    assert!(manager
        .release_player_drop_placement_for_key(&key, &slot)
        .unwrap());
    assert!(replica
        .release_player_drop_placement_for_key(&key, &slot)
        .unwrap());
    assert_eq!(manager.admitted_action_count(), before_admissions);
    assert_eq!(
        manager.checkpoint_bytes().unwrap(),
        before,
        "definite rejection returns the exact pre-reservation world checkpoint"
    );
    assert_eq!(replica.checkpoint_bytes().unwrap(), before);
}
