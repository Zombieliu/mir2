//! Explicit internal full-shared-Tick diagnostic, never a CCU/SLO test.
use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

const POPULATION: usize = 100;
const WORKERS: usize = 4;
const ROUNDS: usize = 3;
const WALL: Duration = Duration::from_secs(120);

#[derive(Default)]
struct ProbeWorker {
    samples: Vec<u64>,
    ticks_called: usize,
    personal_clock_advances: usize,
    turns: usize,
    logout_ok: usize,
    failures: usize,
    expired: bool,
}

fn normal_logout_probe(runtime: &mut ZoneRuntimeHandle) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let result = runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut));
        result.is_ok_and(|packets| {
            packets.iter().any(|p| matches!(p, ServerPacket::LogOutSuccess { .. }))
                && runtime.active_identity().is_none()
        })
    }))
    .unwrap_or(false)
}

fn probe_quantiles(mut samples: Vec<u64>) -> serde_json::Value {
    samples.sort_unstable();
    if samples.is_empty() { return serde_json::json!({"count": 0, "p50_us": null, "p95_us": null, "max_us": null, "samples_us": samples}); }
    let p95 = (samples.len() * 95).div_ceil(100) - 1;
    serde_json::json!({
        "count": samples.len(), "p50_us": samples[(samples.len() - 1) / 2],
        "p95_us": samples[p95], "max_us": samples.last(), "samples_us": samples,
    })
}

#[test]
#[ignore = "explicit100-owner/4-caller memory diagnostic; full shared Tick, not CCU; external120s watchdog required"]
fn probe_full_shared_tick_100_owners_four_callers_memory() {
    assert_eq!(
        std::env::var("MIR2_GATEWAY_SLOW_STAGE_MS").as_deref(),
        Ok("20"),
        "launch only this exact ignored test with the existing20ms diagnostics enabled"
    );
    let began = Instant::now();
    let deadline = began + WALL;
    let config = GatewayConfig::default().with_crystal_world_runtime();
    assert!(config.account_store_path.is_none(), "memory-only diagnostic");
    assert!(config.account_store_database_url.is_none(), "no live repository");
    assert!(config.content_profile.is_none(), "no platinum profile override");
    assert!(config.monster_spawn_source == mir2_simulation::MonsterSpawnSource::CrystalWorld);
    // Clone shares the original account authority; do not create one Store per actor.
    let cloned = config.clone();
    assert!(Arc::ptr_eq(&config.account_store, &cloned.account_store));
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let zone_id = ZoneId::new("tick-leaf-probe-owned-memory");
    let resources = factory.resources_for_zone(&zone_id);
    let mut actors = Vec::with_capacity(POPULATION);
    let mut initialized = 0;
    let mut initialization_failed = false;
    for ordinal in 0..POPULATION {
        if Instant::now() >= deadline {
            initialization_failed = true;
            break;
        }
        let mut runtime = factory.create_runtime(config.clone(), &zone_id);
        let result = catch_unwind(AssertUnwindSafe(|| {
            start_new_runtime_handle(
                &mut runtime,
                &format!("leaf{ordinal:03}"),
                &format!("TickLeaf{ordinal:03}"),
            );
            let shared = runtime.as_any()
                .downcast_ref::<SharedInProcessZoneSessionRuntime>()
                .expect("must use the actual full shared runtime");
            assert!(Arc::ptr_eq(&shared.zone_state, &resources.zone_state));
            assert!(runtime.active_identity().is_some());
        }));
        actors.push(runtime);
        if result.is_err() {
            initialization_failed = true;
            break;
        }
        initialized += 1;
    }
    let initialization_ms = began.elapsed().as_millis();
    let initial_presence = resources.zone_state.lock().unwrap().players.len();
    if initialization_failed || initialized != POPULATION || initial_presence != POPULATION {
        let closed: usize = actors.iter_mut().map(|r| usize::from(normal_logout_probe(r))).sum();
        eprintln!("TICK_LEAF_PROBE_INIT_FAILURE {}", serde_json::json!({
            "initialized": initialized, "expected": POPULATION, "presence": initial_presence,
            "normal_logout_count": closed, "expired": Instant::now() >= deadline,
            "population_reduced": false, "capacity_qualified": false,
        }));
        panic!("full100 initialization failed or exceeded the total diagnostic deadline");
    }
    {
        let state = resources.zone_state.lock().unwrap();
        let authority = state.experience_authority.as_ref()
            .expect("normal admission must bind the shared Store authority");
        assert!(Arc::ptr_eq(&authority.account_store, &config.account_store));
    }
    // Same100 existing actors and one existing Zone owner. This wave allows100
    // in-flight synchronous calls under the caller process's independent CPU Job.
    let burst_barrier = Arc::new(Barrier::new(POPULATION));
    let mut burst_handles = Vec::with_capacity(POPULATION);
    for mut runtime in actors {
        let barrier = burst_barrier.clone();
        burst_handles.push(thread::spawn(move || {
            barrier.wait();
            let called = Instant::now() < deadline;
            let before = runtime.current_tick();
            let started = Instant::now();
            let ok = called && catch_unwind(AssertUnwindSafe(|| {
                runtime.execute(WorldCommand::Tick).is_ok()
            })).unwrap_or(false);
            let elapsed = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            let advanced = runtime.current_tick() > before;
            (runtime, called, ok, elapsed, advanced)
        }));
    }
    let mut actors = Vec::with_capacity(POPULATION);
    let (mut burst_calls, mut burst_ok, mut burst_advances) = (0usize, 0usize, 0usize);
    let mut burst_samples = Vec::with_capacity(POPULATION);
    for handle in burst_handles {
        let (runtime, called, ok, elapsed, advanced) = handle.join().expect("burst caller must join");
        burst_calls += usize::from(called);
        burst_ok += usize::from(ok);
        burst_advances += usize::from(advanced);
        if ok { burst_samples.push(elapsed); }
        actors.push(runtime);
    }
    let burst_joined = actors.len();
    let burst_failed = burst_calls != POPULATION || burst_ok != POPULATION;
    eprintln!("FULL_SHARED_TICK_BURST {}", serde_json::json!({
        "population": POPULATION, "callers_joined": burst_joined, "actual_tick_calls": burst_calls,
        "successful_tick_calls": burst_ok, "personal_clock_advances": burst_advances,
        "diagnostic_only": true, "capacity_qualified": false,
        "elapsed_since_test_start_ms": began.elapsed().as_millis(),
        "outer_tick": probe_quantiles(burst_samples),
    }));
    let mut groups: Vec<Vec<ZoneRuntimeHandle>> = (0..WORKERS).map(|_| Vec::new()).collect();
    for (index, runtime) in actors.into_iter().enumerate() {
        groups[index % WORKERS].push(runtime);
    }
    let stop = Arc::new(AtomicBool::new(burst_failed));
    let barrier = Arc::new(Barrier::new(WORKERS));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::with_capacity(WORKERS);
    for (worker, mut group) in groups.into_iter().enumerate() {
        let stop = stop.clone();
        let barrier = barrier.clone();
        let sender = sender.clone();
        handles.push(thread::spawn(move || {
            let mut result = ProbeWorker::default();
            barrier.wait();
            let execution = catch_unwind(AssertUnwindSafe(|| {
                'rounds: for round in 0..ROUNDS {
                    for (index, runtime) in group.iter_mut().enumerate() {
                        if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
                            result.expired = Instant::now() >= deadline;
                            break 'rounds;
                        }
                        // Ordinary Turn only: no teleport, synthetic movement grant or lock hold.
                        // Only one actor per worker turns; this is not movement-load parity.
                        if index == 0 {
                            let direction = if round % 2 == 0 { MirDirection::Right } else { MirDirection::Down };
                            if runtime.execute(WorldCommand::ClientPacket(ClientPacket::Turn { direction })).is_err() {
                                result.failures += 1;
                                stop.store(true, Ordering::Release);
                                break 'rounds;
                            }
                            result.turns += 1;
                        }
                        let before = runtime.current_tick();
                        let started = Instant::now();
                        // Crucial boundary: full SharedInProcessZoneSessionRuntime pipeline,
                        // including predrain, vital basis, inner personal Tick and reconcile.
                        let tick = runtime.execute(WorldCommand::Tick);
                        let elapsed = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                        result.ticks_called += 1;
                        if tick.is_err() {
                            result.failures += 1;
                            stop.store(true, Ordering::Release);
                            break 'rounds;
                        }
                        result.samples.push(elapsed);
                        result.personal_clock_advances += usize::from(runtime.current_tick() > before);
                    }
                }
            }));
            if execution.is_err() { result.failures += 1; stop.store(true, Ordering::Release); }
            // Deadline is cooperative, never a claim that Rust can cancel a blocked mutex.
            // Cleanup is still attempted once per owned actor, with no command retry.
            for runtime in &mut group {
                if normal_logout_probe(runtime) { result.logout_ok += 1; }
                else { result.failures += 1; }
            }
            result.expired |= Instant::now() >= deadline;
            let _ = sender.send((worker, result));
        }));
    }
    drop(sender);
    let mut results = Vec::with_capacity(WORKERS);
    let mut deadline_failed = false;
    for _ in 0..WORKERS {
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(result) => results.push(result),
            Err(_) => { deadline_failed = true; stop.store(true, Ordering::Release); break; }
        }
    }
    // No detached successful return: synchronous stalls cannot be forcibly cancelled.
    // An external owned Job must record timeout/failure and retain custody until exit.
    let joined = handles.into_iter().map(|h| h.join().is_ok()).filter(|ok| *ok).count();
    results.extend(receiver.try_iter());
    let remaining_presence = resources.zone_state.lock().unwrap().players.len();
    let calls: usize = results.iter().map(|(_, r)| r.ticks_called).sum();
    let normal_logouts: usize = results.iter().map(|(_, r)| r.logout_ok).sum();
    let advances: usize = results.iter().map(|(_, r)| r.personal_clock_advances).sum();
    let failures: usize = results.iter().map(|(_, r)| r.failures).sum();
    let turns: usize = results.iter().map(|(_, r)| r.turns).sum();
    let expired = deadline_failed || results.iter().any(|(_, r)| r.expired) || began.elapsed() >= WALL;
    let samples = results.iter_mut().flat_map(|(_, r)| std::mem::take(&mut r.samples)).collect();
    eprintln!("FULL_SHARED_TICK_PROBE {}", serde_json::json!({
        "schema": "mir2.native-full-shared-tick-probe.v1", "diagnostic_only": true,
        "capacity_qualified": false, "backend": "memory", "content_profile": "crystal_full",
        "level_equipment": "ordinary_new_character_defaults; differs fromL40/SharpBracelet fixture",
        "map": config.map.file_name, "population": POPULATION, "initial_presence": initial_presence,
        "caller_workers": WORKERS, "rounds": ROUNDS, "actual_tick_calls": calls,
        "preceding_burst_tick_calls": burst_calls, "preceding_burst_callers_joined": burst_joined,
        "personal_clock_advances": advances, "ordinary_turns": turns,
        "normal_logouts": normal_logouts, "remaining_presence": remaining_presence,
        "caller_workers_joined": joined, "failures": failures, "deadline_failed": expired,
        "initialization_ms": initialization_ms, "whole_diagnostic_ms": began.elapsed().as_millis(),
        "debug_assertions": cfg!(debug_assertions), "outer_tick": probe_quantiles(samples),
        "socket_write_gate_or_network_measured": false, "artificial_lock_hold": false,
    }));
    assert_eq!(burst_joined, POPULATION);
    assert_eq!(burst_calls, POPULATION);
    assert_eq!(burst_ok, POPULATION);
    assert_eq!(joined, WORKERS, "every caller thread must join");
    assert_eq!(results.len(), WORKERS);
    assert_eq!(failures, 0, "normal execution/cleanup failed");
    assert_eq!(normal_logouts, POPULATION);
    assert_eq!(remaining_presence, 0);
    assert_eq!(calls, POPULATION * ROUNDS, "no omitted or reduced population/rounds");
    assert!(advances > 0, "diagnostic must reach actual inner personal Tick, not only grace");
    assert!(!expired, "entire diagnostic exceeded120s; never reported as a smaller successful cohort");
}
