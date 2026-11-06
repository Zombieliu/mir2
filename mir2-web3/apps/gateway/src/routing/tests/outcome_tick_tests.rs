use super::*;
use mir2_protocol::MirGridType;

#[test]
fn shared_current_tick_reads_the_personal_clock_without_the_zone_snapshot_lock() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let runtime = shared_session_runtime(shared.clone());
    let expected = runtime.inner.world_snapshot().tick;
    // Poisoning this isolated unused Zone detects an accidental full shared
    // snapshot without leaving a test blocked on a recursively acquired lock.
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = shared.lock().unwrap();
        panic!("isolated projection lock probe");
    }));
    assert!(poisoned.is_err());
    let projected =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.current_tick()));
    shared.clear_poison();
    assert_eq!(
        projected.expect("clock projection must not compose or lock shared AOI"),
        expected
    );
    assert_eq!(runtime.inner.world_snapshot().tick, expected);
}

#[test]
fn ordinary_item_outcome_retains_tick_state_and_packets_for_all_three_classes() {
    for (label, class) in [
        ("warrior", MirClass::Warrior),
        ("wizard", MirClass::Wizard),
        ("taoist", MirClass::Taoist),
    ] {
        let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(shared);
        start_new_runtime_with_class(&mut runtime, &format!("tick-{label}"), "TickReader", class);
        runtime.execute(WorldCommand::Tick).unwrap();
        let before = runtime.inner.world_snapshot();
        assert!(before.tick > 0);
        let execution = runtime
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::UseItem {
                unique_id: u64::MAX,
                grid: MirGridType::Inventory,
            }))
            .unwrap();
        let after = runtime.world_snapshot();
        assert_eq!(
            execution.outcome.command_kind,
            mir2_simulation::WorldCommandKind::ClientPacket("UseItem")
        );
        assert_eq!(execution.outcome.snapshot_tick, after.tick);
        assert_eq!(after.tick, runtime.inner.current_tick());
        assert_eq!(execution.outcome.packet_count, execution.packets.len());
        assert_eq!(
            execution.outcome.active_identity,
            runtime.inner.active_identity()
        );
        assert_eq!(before.inventory_items, after.inventory_items);
        assert_eq!(before.gold, after.gold);
        assert_eq!(before.map_file_name, after.map_file_name);
        assert!(execution.packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UseItem {
                unique_id: u64::MAX,
                success: false,
                ..
            }
        )));
        assert!(execution.game_shop_purchase_outcome.is_none());
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .unwrap();
    }
}

#[test]
fn shared_projection_keeps_existing_movement_skip_and_rejects_unauthenticated_start() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    start_new_runtime(&mut runtime, "tick-movement", "TickWalker");
    runtime.execute(WorldCommand::Tick).unwrap();
    let before = runtime.inner.current_tick();
    assert!(before > 0);
    let rejected = runtime.execute_production_player_command(
        false,
        WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
    );
    assert!(rejected.is_err());
    assert_eq!(runtime.inner.current_tick(), before);
    let execution = runtime
        .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Walk {
            direction: MirDirection::Right,
        }))
        .unwrap();
    assert_eq!(execution.outcome.snapshot_tick, 0);
    assert_eq!(execution.outcome.packet_count, execution.packets.len());
    assert_eq!(runtime.current_tick(), runtime.world_snapshot().tick);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
}
