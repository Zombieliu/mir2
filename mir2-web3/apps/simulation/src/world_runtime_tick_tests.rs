use super::*;
use mir2_protocol::MirGridType;
use std::sync::atomic::{AtomicUsize, Ordering};

fn started() -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .unwrap();
    runtime
}

/// A runtime that has not opted into the projection retains the old fallback.
struct LegacyRuntime {
    inner: InProcessWorldRuntime,
    snapshot_reads: AtomicUsize,
}

impl WorldRuntime for LegacyRuntime {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.inner.on_connect()
    }
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.inner.execute(command)
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        self.snapshot_reads.fetch_add(1, Ordering::Relaxed);
        self.inner.world_snapshot()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.inner.active_identity()
    }
    fn save_active_character(&mut self) -> Result<(), String> {
        self.inner.save_active_character()
    }
    fn refresh_active_external_mail(&mut self) -> bool {
        self.inner.refresh_active_external_mail()
    }
}

#[test]
fn legacy_current_tick_fallback_retains_the_actual_snapshot_and_outcome() {
    let mut runtime = LegacyRuntime {
        inner: started(),
        snapshot_reads: AtomicUsize::new(0),
    };
    let execution = runtime
        .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: u64::MAX,
            grid: MirGridType::Inventory,
        }))
        .unwrap();
    assert_eq!(runtime.snapshot_reads.load(Ordering::Relaxed), 1);
    assert_eq!(
        execution.outcome.snapshot_tick,
        runtime.inner.world_snapshot().tick
    );
    assert_eq!(execution.outcome.packet_count, execution.packets.len());
    assert_eq!(
        execution.outcome.active_identity,
        runtime.inner.active_identity()
    );
    assert!(execution.game_shop_purchase_outcome.is_none());
    assert!(execution.packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UseItem {
            unique_id: u64::MAX,
            success: false,
            ..
        }
    )));
}

#[test]
fn current_tick_reads_are_pure_before_start_during_play_and_after_logout() {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    assert_eq!(runtime.current_tick(), runtime.world_snapshot().tick);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .unwrap();
    for _ in 0..3 {
        runtime.execute(WorldCommand::Tick).unwrap();
        let before = runtime.world_snapshot();
        for _ in 0..32 {
            assert_eq!(runtime.current_tick(), before.tick);
        }
        assert_eq!(runtime.world_snapshot(), before);
    }
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    let before = runtime.world_snapshot();
    assert_eq!(runtime.current_tick(), before.tick);
    assert_eq!(runtime.world_snapshot(), before);
}

#[test]
fn rejected_use_item_keeps_real_tick_identity_inventory_and_acknowledgement() {
    let mut runtime = started();
    runtime.execute(WorldCommand::Tick).unwrap();
    let before = runtime.world_snapshot();
    assert!(before.tick > 0);
    let execution = runtime
        .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: u64::MAX,
            grid: MirGridType::Inventory,
        }))
        .unwrap();
    let after = runtime.world_snapshot();
    assert_eq!(execution.outcome.snapshot_tick, after.tick);
    assert_eq!(execution.outcome.active_identity, runtime.active_identity());
    assert_eq!(execution.outcome.packet_count, execution.packets.len());
    assert_eq!(after.inventory_items, before.inventory_items);
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.map_file_name, before.map_file_name);
    assert!(execution.packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UseItem {
            unique_id: u64::MAX,
            success: false,
            ..
        }
    )));
    assert!(execution.game_shop_purchase_outcome.is_none());
}

#[test]
fn projected_outcome_preserves_existing_movement_skip_and_authentication_guard() {
    let mut runtime = started();
    runtime.execute(WorldCommand::Tick).unwrap();
    let tick = runtime.current_tick();
    assert!(tick > 0);
    let rejected = runtime.execute_production_player_command(
        false,
        WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
    );
    assert!(rejected.is_err());
    assert_eq!(runtime.current_tick(), tick);
    let execution = runtime
        .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Walk {
            direction: MirDirection::Right,
        }))
        .unwrap();
    assert_eq!(execution.outcome.snapshot_tick, 0);
    assert_eq!(execution.outcome.packet_count, execution.packets.len());
    assert_eq!(runtime.current_tick(), runtime.world_snapshot().tick);
}
