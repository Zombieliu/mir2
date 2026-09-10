use super::*;

#[test]
fn creature_mouse_pickup_rejects_remote_tile_and_baby_pig_can_pick_nearby() {
    let mut creature = shared_pickup_creature();
    creature.pet_type = 0;
    creature.creature_rules.mouse_pickup_range = 3;
    creature.creature_rules.auto_pickup_enabled = false;
    let (mut first, mut second) = started_shared_zone_sessions_with_creature(creature.clone());
    first.handle_packet(ClientPacket::DropGold { amount: 100 });
    let drop = second.world_snapshot().ground_drops[0].clone();
    second.transfer_map(&format!("crystal:0:{}:{}", drop.x + 10, drop.y));
    second.handle_packet(ClientPacket::UpdateIntelligentCreature {
        creature,
        summon_me: true,
        unsummon_me: false,
        release_me: false,
    });
    let gold = second.world_snapshot().gold;
    let packets = second.handle_packet(ClientPacket::IntelligentCreaturePickup {
        mouse_mode: true,
        location: Point {
            x: drop.x,
            y: drop.y,
        },
    });
    assert!(!packets.iter().any(|p| matches!(
        p,
        ServerPacket::GainedGold { .. } | ServerPacket::IntelligentCreaturePickup { .. }
    )));
    assert_eq!(second.world_snapshot().gold, gold);
    assert!(first
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|d| d.object_id == drop.object_id));
    second.transfer_map(&format!("crystal:0:{}:{}", drop.x + 1, drop.y));
    // The shared creature must physically follow its owner before a new mouse request.
    for _ in 0..80 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        second.tick();
    }
    let mut packets = second.handle_packet(ClientPacket::IntelligentCreaturePickup {
        mouse_mode: true,
        location: Point {
            x: drop.x,
            y: drop.y,
        },
    });
    packets.extend(wait_for_shared_creature_pickup(&mut second));
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::GainedGold { gold: 100 })),
        "active={:?}, packets={packets:?}",
        second
            .world_snapshot()
            .stage5_systems
            .active_intelligent_creature()
    );
    assert_eq!(second.world_snapshot().gold, gold + 100);
}

#[test]
fn creature_failed_operation_wallet_save_recovers_after_normal_relogin_once() {
    fn pending_operation_count(
        zone: &Arc<Mutex<SharedInProcessZoneState>>,
        identity: &mir2_simulation::ActiveSessionIdentity,
    ) -> usize {
        zone.lock()
            .unwrap()
            .zone_manager
            .peek_intelligent_creature_operations_for_identity(
                &identity.account_id,
                identity.character_index,
            )
            .len()
    }
    let config = GatewayConfig::default();
    let zone = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(zone.clone());
    runtime.inner = InProcessWorldRuntime::new(config.clone());
    start_new_runtime(&mut runtime, "pet-reconnect", "Keeper");
    let identity = runtime.inner.active_identity().unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut(&identity.account_id)
            .unwrap()
            .saves
            .get_mut(&identity.character_index)
            .unwrap();
        save.gold = 100;
        let mut state: mir2_simulation::Stage5SystemsState =
            serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
        state.intelligent_creatures = vec![shared_pickup_creature()];
        state.summoned_intelligent_creature_type = Some(99);
        state.intelligent_creature_operations = 999;
        save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    }
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: identity.character_index,
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(
            ClientPacket::UpdateIntelligentCreature {
                creature: shared_pickup_creature(),
                summon_me: true,
                unsummon_me: false,
                release_me: false,
            },
        ))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
            amount: 10,
        }))
        .unwrap();
    runtime.sync_shared_intelligent_creature_actor();
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms();
    for step in 1..=8 {
        runtime.dispatch_zone_player_command(
            ZoneCommand::Tick {
                now_ms: now + step * 1000,
            },
            false,
        );
        if pending_operation_count(&zone, &identity) != 0 {
            break;
        }
    }
    assert_eq!(pending_operation_count(&zone, &identity), 1);
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    runtime.drain_shared_intelligent_creature_actor();
    assert_eq!(
        runtime
            .inner
            .world_snapshot()
            .stage5_systems
            .intelligent_creature_pearls,
        0
    );
    assert_eq!(pending_operation_count(&zone, &identity), 1);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    drop(runtime);
    let mut resumed = shared_session_runtime(zone.clone());
    resumed.inner = InProcessWorldRuntime::new(config.clone());
    resumed
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: identity.account_id.clone(),
            password: identity.account_id.clone(),
        }))
        .unwrap();
    resumed
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: identity.character_index,
        }))
        .unwrap();
    resumed.execute(WorldCommand::Tick).unwrap();
    assert_eq!(
        resumed
            .inner
            .world_snapshot()
            .stage5_systems
            .intelligent_creature_pearls,
        1
    );
    assert!(pending_operation_count(&zone, &identity) == 0);
    resumed.execute(WorldCommand::Tick).unwrap();
    assert_eq!(
        resumed
            .inner
            .world_snapshot()
            .stage5_systems
            .intelligent_creature_pearls,
        1
    );
}
