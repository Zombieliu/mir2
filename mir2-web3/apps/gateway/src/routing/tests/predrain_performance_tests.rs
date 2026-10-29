use super::*;

fn full_world_config() -> GatewayConfig {
    GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile()
}

#[test]
fn predrain_map_name_read_matches_snapshot_through_start_transfer_logout() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    runtime.inner = InProcessWorldRuntime::new(full_world_config());
    assert_eq!(runtime.inner.current_map_file_name(), None);
    assert_eq!(
        runtime.inner.current_map_file_name(),
        runtime.inner.world_snapshot().map_file_name,
    );
    assert!(runtime
        .sync_newly_active_private_monsters_to_zone()
        .is_empty());

    start_new_runtime(&mut runtime, "predrain-map-reader", "MapReader");
    assert_eq!(runtime.inner.current_map_file_name().as_deref(), Some("0"));
    let before = runtime.inner.world_snapshot();
    assert_eq!(runtime.inner.current_map_file_name(), before.map_file_name);
    assert_eq!(
        runtime.inner.world_snapshot(),
        before,
        "the getter must not mutate state"
    );

    let packets = runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:D401:24:181".to_string(),
        })
        .expect("trusted fixture transfer should execute");
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == "D401"
    )));
    assert_eq!(
        runtime.inner.current_map_file_name().as_deref(),
        Some("D401")
    );
    assert_eq!(
        runtime.inner.current_map_file_name(),
        runtime.inner.world_snapshot().map_file_name,
    );
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .expect("normal logout should execute");
    assert_eq!(runtime.inner.current_map_file_name(), None);
    assert_eq!(
        runtime.inner.current_map_file_name(),
        runtime.inner.world_snapshot().map_file_name,
    );
    assert!(runtime
        .sync_newly_active_private_monsters_to_zone()
        .is_empty());
}

/// Mirrors the real hydration gate's old expression, including construction and
/// destruction of the complete owned snapshot. This is intentionally test-only.
fn legacy_hydration_map_name(runtime: &InProcessWorldRuntime) -> Option<String> {
    runtime.world_snapshot().map_file_name
}

#[test]
#[ignore = "explicit release microbenchmark; requires isolated MIR2_QUEST_CADENCE=newcomer-v2"]
fn predrain_map_name_fifteen_native_sessions_release_benchmark() {
    assert!(!cfg!(debug_assertions), "run this benchmark with --release");
    assert_eq!(
        std::env::var("MIR2_QUEST_CADENCE").as_deref(),
        Ok("newcomer-v2")
    );
    const SESSION_COUNT: usize = 15;
    const ROUNDS: usize = 50;
    const TRIALS: usize = 5;

    // Use the baseline's CrystalWorld/platinum176 data, actual native account /
    // character / StartGame commands, and one shared Zone. No sockets or live
    // persistence are involved. This times one hydration lookup, not capacity,
    // network latency, allocator bytes or the entire movement path.
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let config = full_world_config();
    let mut runtimes = Vec::with_capacity(SESSION_COUNT);
    for index in 0..SESSION_COUNT {
        let mut runtime = shared_session_runtime(shared.clone());
        runtime.inner = InProcessWorldRuntime::new(config.clone());
        let class = [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist][index % 3];
        start_new_runtime_with_class(
            &mut runtime,
            &format!("map-read-bench-{index:02}"),
            &format!("MapBench{index:02}"),
            class,
        );
        assert_eq!(runtime.inner.current_map_file_name().as_deref(), Some("0"));
        runtimes.push(runtime);
    }
    assert_eq!(shared.lock().unwrap().players.len(), SESSION_COUNT);

    // Report observed top-level projection counts instead of treating JSON
    // length or Rust size_of as a heap measurement. Each old call also clones
    // nested strings, tooltips and Stage5 state that these counts do not include.
    let mut entity_records = 0;
    let mut item_records = 0;
    let mut quest_records = 0;
    let mut transfer_records = 0;
    for runtime in &runtimes {
        let snapshot = runtime.inner.world_snapshot();
        assert_eq!(
            runtime.inner.current_map_file_name(),
            snapshot.map_file_name
        );
        entity_records += snapshot.entities.len();
        item_records += snapshot.belt_items.len()
            + snapshot.inventory_items.len()
            + snapshot.storage_items.len()
            + snapshot.equipment_items.len()
            + snapshot.hero_inventory_items.len()
            + snapshot.hero_equipment_items.len();
        quest_records += snapshot.quest_log.len();
        transfer_records += snapshot.map_transfers.len();
        assert_eq!(
            legacy_hydration_map_name(&runtime.inner),
            runtime.inner.current_map_file_name()
        );
    }

    let time_reads = |legacy: bool| {
        let started = Instant::now();
        let mut checksum = 0usize;
        for _ in 0..ROUNDS {
            for runtime in &runtimes {
                let inner = std::hint::black_box(&runtime.inner);
                let name = if legacy {
                    legacy_hydration_map_name(inner)
                } else {
                    inner.current_map_file_name()
                };
                checksum += std::hint::black_box(name).expect("started map").len();
            }
        }
        (started.elapsed(), checksum)
    };
    let mut old_times = Vec::with_capacity(TRIALS);
    let mut new_times = Vec::with_capacity(TRIALS);
    for trial in 0..TRIALS {
        // Alternate ordering so only one branch does not always run cold/first.
        let (old, new) = if trial % 2 == 0 {
            (time_reads(true), time_reads(false))
        } else {
            let new = time_reads(false);
            (time_reads(true), new)
        };
        assert_eq!(old.1, SESSION_COUNT * ROUNDS);
        assert_eq!(old.1, new.1);
        old_times.push(old.0);
        new_times.push(new.0);
        eprintln!(
            "predrain-map-name trial={} sessions={} reads={} old_us={} new_us={}",
            trial + 1,
            SESSION_COUNT,
            SESSION_COUNT * ROUNDS,
            old.0.as_micros(),
            new.0.as_micros(),
        );
    }
    old_times.sort();
    new_times.sort();
    eprintln!(
        "predrain-map-name median reads={} old_us={} new_us={} old_full_snapshot_builds={} new_full_snapshot_builds=0 old_entity_projections={} old_item_projections={} old_quest_projections={} old_transfer_projections={} new_map_name_clones={}; counts are logical records, not heap bytes or capacity",
        SESSION_COUNT * ROUNDS,
        old_times[TRIALS / 2].as_micros(), new_times[TRIALS / 2].as_micros(),
        SESSION_COUNT * ROUNDS, entity_records * ROUNDS, item_records * ROUNDS,
        quest_records * ROUNDS, transfer_records * ROUNDS, SESSION_COUNT * ROUNDS,
    );
    // Deliberately no timing threshold: equivalence is a correctness gate;
    // timings are evidence to compare with the independent public-server run.
}
