use super::*;
use super::super::shared_experience_profiles::{
    owner_profile_publications_for_test, set_projection_test_hook, ProjectionTestHook, ProjectionTestPhase,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;

#[derive(Default)]
struct ProbeState { entries: usize, owners: usize, scans: usize, first_waiting: bool, release: bool, timed_out: bool }
#[derive(Default)]
struct Probe { state: Mutex<ProbeState>, changed: Condvar, owner_barrier: bool, first_publication_race: bool }

impl Probe {
    fn hook(self: &Arc<Self>) -> ProjectionTestHook {
        let this = self.clone();
        Arc::new(move |phase| {
            let mut state = this.state.lock().unwrap();
            match phase {
                ProjectionTestPhase::Entry => {
                    state.entries += 1;
                    let second_entry = state.entries == 2;
                    this.changed.notify_all();
                    let (next, timeout) = this.changed.wait_timeout_while(state, Duration::from_secs(5), |s| s.entries < 2 && !s.timed_out).unwrap();
                    state = next;
                    if timeout.timed_out() { state.timed_out = true; this.changed.notify_all(); }
                    // Negative premise: second owner's first publication occurs only
                    // after the leader has captured its scan signature.
                    if this.first_publication_race && second_entry {
                        let (next, timeout) = this.changed.wait_timeout_while(state, Duration::from_secs(5), |s| !s.first_waiting && !s.timed_out).unwrap();
                        state = next;
                        if timeout.timed_out() { state.timed_out = true; this.changed.notify_all(); }
                    }
                }
                ProjectionTestPhase::OwnerPublished => {
                    state.owners += 1; this.changed.notify_all();
                    // Positive premise: both fresh owner publications must finish
                    // before either caller captures its gate signature.
                    if this.owner_barrier {
                        let (next, timeout) = this.changed.wait_timeout_while(state, Duration::from_secs(5), |s| s.owners < 2 && !s.timed_out).unwrap();
                        state = next;
                        if timeout.timed_out() { state.timed_out = true; this.changed.notify_all(); }
                    }
                }
                ProjectionTestPhase::ScanStarted => {
                    state.scans += 1;
                    if state.scans == 1 {
                        // Called after the old per-peer guards have not yet been taken.
                        // The only held guard is this test's own condition-variable state.
                        state.first_waiting = true;
                        this.changed.notify_all();
                        let (next, timeout) = this.changed.wait_timeout_while(state, Duration::from_secs(5), |s| !s.release && !s.timed_out).unwrap();
                        state = next;
                        if timeout.timed_out() { state.timed_out = true; this.changed.notify_all(); }
                    }
                }
                ProjectionTestPhase::ScanFinished => {}
            }
        })
    }
}

fn overlap_pair(prepare: impl FnOnce(&mut [ZoneRuntimeHandle], &GatewayConfig)) -> (usize, bool) {
    overlap_pair_with_premise(prepare, true)
}

fn overlap_pair_with_premise(prepare: impl FnOnce(&mut [ZoneRuntimeHandle], &GatewayConfig), same_generation: bool) -> (usize, bool) {
    let config = GatewayConfig::default();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let zone = ZoneId::new("projection-overlap-owned");
    let mut actors = Vec::new();
    for ordinal in 0..2 {
        let mut runtime = factory.create_runtime(config.clone(), &zone);
        start_new_runtime_handle(&mut runtime, &format!("proj{ordinal}"), &format!("ProjNative{ordinal}"));
        actors.push(runtime);
    }
    prepare(&mut actors, &config);
    let resources = factory.resources_for_zone(&zone);
    assert_eq!(resources.zone_state.lock().unwrap().players.len(), 2);
    let shared = actors[0].as_any_mut().downcast_mut::<SharedInProcessZoneSessionRuntime>().unwrap();
    // A real shared generation invalidates both existing per-runtime caches.
    shared.shared_mentors.lock().unwrap().changed();
    let probe = Arc::new(Probe { owner_barrier: same_generation, first_publication_race: !same_generation, ..Probe::default() });
    let (done_tx, done_rx) = mpsc::channel();
    let mut handles = Vec::new();
    for mut runtime in actors {
        let hook = probe.hook();
        let done = done_tx.clone();
        handles.push(thread::spawn(move || {
            set_projection_test_hook(Some(hook));
            let before = owner_profile_publications_for_test();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                runtime.as_any_mut().downcast_mut::<SharedInProcessZoneSessionRuntime>().unwrap()
                    .refresh_shared_experience_profiles()
            }));
            let publications = owner_profile_publications_for_test() - before;
            set_projection_test_hook(None);
            let success = matches!(outcome, Ok(Ok(())));
            let _ = done.send(());
            (runtime, success, publications)
        }));
    }
    drop(done_tx);
    let state = probe.state.lock().unwrap();
    let (state, timeout) = probe.changed.wait_timeout_while(state, Duration::from_secs(5), |s| !(s.first_waiting && s.owners == 2) && !s.timed_out).unwrap();
    let reached = state.first_waiting && state.owners == 2 && !timeout.timed_out() && !state.timed_out;
    drop(state);
    let returned_before_completion = done_rx.recv_timeout(Duration::from_millis(50)).is_ok();
    // Release on every path BEFORE assertions or joins; timeout cannot strand our hook.
    { let mut state = probe.state.lock().unwrap(); state.release = true; probe.changed.notify_all(); }
    let mut joined = Vec::new();
    for handle in handles { joined.push(handle.join()); }
    let mut successful = 0;
    let mut publications = 0;
    let mut logouts = 0;
    for result in joined.iter_mut() {
        if let Ok((runtime, ok, count)) = result {
            successful += usize::from(*ok);
            publications += *count;
            let packets = runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut));
            logouts += usize::from(packets.is_ok_and(|p| p.iter().any(|p| matches!(p, ServerPacket::LogOutSuccess { .. }))) && runtime.active_identity().is_none());
        }
    }
    let state = probe.state.lock().unwrap();
    let scans = state.scans;
    let timed_out = state.timed_out;
    drop(state);
    let remaining = resources.zone_state.lock().unwrap().players.len();
    assert!(reached && !timed_out, "test synchronization did not establish two overlapping callers");
    assert!(joined.iter().all(Result::is_ok), "both native caller threads must join");
    assert_eq!(successful, 2);
    assert_eq!(publications, 2, "each caller must always publish its own fresh profile");
    assert_eq!(logouts, 2);
    assert_eq!(remaining, 0, "normal cleanup precedes the expected RED assertion");
    (scans, returned_before_completion)
}


#[test]
fn overlapping_projection_waits_for_complete_scan_but_publishes_each_owner() {
    let (scans, early) = overlap_pair(|_, _| {});
    assert_eq!(scans, 1, "overlapping callers must share one complete projection");
    assert!(!early, "waiter cannot reuse an unfinished projection");
}

#[test]
fn first_owner_publication_generation_change_requires_two_scans() {
    let (scans, early) = overlap_pair_with_premise(|_, _| {}, false);
    assert_eq!(scans, 2, "do not coalesce when the first scan's generation changes");
    assert!(!early, "waiter must wait even though the leader's result cannot be reused");
}

fn shared(runtime: &mut ZoneRuntimeHandle) -> &mut SharedInProcessZoneSessionRuntime {
    runtime.as_any_mut().downcast_mut::<SharedInProcessZoneSessionRuntime>().unwrap()
}

#[test]
fn different_store_authorities_never_share_overlap_completion() {
    let (scans, _) = overlap_pair(|actors, config| {
        let mut independent = config.clone();
        independent.account_store = Arc::new(Mutex::new(config.account_store.lock().unwrap().clone()));
        assert!(!config.same_shared_experience_projection_authority(&independent));
        shared(&mut actors[1]).inner.rebind_account_store(&independent);
    });
    assert_eq!(scans, 2);
}

#[test]
fn same_store_different_real_clock_authority_does_not_share_completion() {
    let (scans, _) = overlap_pair(|actors, config| {
        let mut independent_clock = config.fork_with_isolated_account_store().unwrap();
        independent_clock.account_store = config.account_store.clone();
        assert!(Arc::ptr_eq(&config.account_store, &independent_clock.account_store));
        assert!(!config.same_shared_experience_projection_authority(&independent_clock));
        shared(&mut actors[1]).inner.rebind_account_store(&independent_clock);
    });
    assert_eq!(scans, 2);
}

#[test]
fn actual_zone_and_replica_registry_domains_are_part_of_overlap_key() {
    for replace_replica_domain in [false, true] {
        let (scans, _) = overlap_pair(|actors, _| {
            let second = shared(&mut actors[1]);
            if replace_replica_domain {
                let copy = second.ranking_replica_zones.lock().unwrap().clone();
                second.ranking_replica_zones = Arc::new(Mutex::new(copy));
            } else {
                let copy = second.ranking_zones.lock().unwrap().clone();
                second.ranking_zones = Arc::new(Mutex::new(copy));
            }
        });
        assert_eq!(scans, 2, "equal registry contents are not the same recipient domain");
    }
}

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct NativeFixture {
    config: GatewayConfig,
    _factory: SharedInProcessZoneRuntimeFactory,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    actors: Vec<ZoneRuntimeHandle>,
}
fn fixture(label: &str, count: usize) -> NativeFixture {
    let config = GatewayConfig::default();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let zone = ZoneId::new(label);
    let mut actors = Vec::new();
    for ordinal in 0..count {
        let mut runtime = factory.create_runtime(config.clone(), &zone);
        start_new_runtime_handle(&mut runtime, &format!("{label}{ordinal}"), &format!("{label}P{ordinal}"));
        actors.push(runtime);
    }
    let zone_state = factory.resources_for_zone(&zone).zone_state.clone();
    shared(&mut actors[0]).shared_mentors.lock().unwrap().changed();
    NativeFixture { config, _factory: factory, zone_state, actors }
}
fn count_hook(scans: Arc<AtomicUsize>, owners: Arc<AtomicUsize>) -> ProjectionTestHook {
    Arc::new(move |phase| match phase {
        ProjectionTestPhase::ScanStarted => { scans.fetch_add(1, Ordering::Relaxed); }
        ProjectionTestPhase::OwnerPublished => { owners.fetch_add(1, Ordering::Relaxed); }
        _ => {}
    })
}
fn refresh(runtime: &mut ZoneRuntimeHandle, hook: ProjectionTestHook) -> Result<(), String> {
    set_projection_test_hook(Some(hook));
    let result = shared(runtime).refresh_shared_experience_profiles();
    set_projection_test_hook(None);
    result
}
fn close_fixture(fixture: &mut NativeFixture) -> usize {
    fixture.actors.iter_mut().map(|runtime| {
        let result = runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut));
        usize::from(result.is_ok_and(|p| p.iter().any(|p| matches!(p, ServerPacket::LogOutSuccess { .. }))) && runtime.active_identity().is_none())
    }).sum()
}
fn replace_stage5(fixture: &NativeFixture, ordinal: usize, value: Option<String>) -> Option<String> {
    let identity = fixture.actors[ordinal].active_identity().unwrap();
    let mut store = fixture.config.account_store.lock().unwrap();
    let save = store.accounts.get_mut(&identity.account_id).unwrap().saves.get_mut(&identity.character_index).unwrap();
    std::mem::replace(&mut save.stage5_systems_json, value)
}

#[test]
fn sequential_cold_caller_still_detects_unrelated_peer_parse_error_and_retries() {
    let mut f = fixture("cold", 2);
    let scans = Arc::new(AtomicUsize::new(0)); let owners = Arc::new(AtomicUsize::new(0));
    let hook = count_hook(scans.clone(), owners.clone());
    let first = refresh(&mut f.actors[0], hook.clone());
    let original = replace_stage5(&f, 0, Some("{invalid-peer-stage5".into()));
    let cold = refresh(&mut f.actors[1], hook.clone());
    replace_stage5(&f, 0, original);
    let retry = refresh(&mut f.actors[1], hook);
    let closed = close_fixture(&mut f);
    assert_eq!(closed, 2); assert_eq!(f.zone_state.lock().unwrap().players.len(), 0);
    assert!(first.is_ok() && cold.is_err() && retry.is_ok());
    assert_eq!(scans.load(Ordering::Relaxed), 3, "later cold caller must not reuse a historical completion");
    assert_eq!(owners.load(Ordering::Relaxed), 3);
}

#[test]
fn stable_local_cache_keeps_original_shortcut_but_always_publishes_owner() {
    let mut f = fixture("local", 2);
    let scans = Arc::new(AtomicUsize::new(0)); let owners = Arc::new(AtomicUsize::new(0));
    let hook = count_hook(scans.clone(), owners.clone());
    let first = refresh(&mut f.actors[0], hook.clone());
    let local = refresh(&mut f.actors[0], hook);
    let closed = close_fixture(&mut f);
    assert_eq!(closed, 2); assert!(first.is_ok() && local.is_ok());
    assert_eq!(scans.load(Ordering::Relaxed), 1, "preserve the old local-cache behavior");
    assert_eq!(owners.load(Ordering::Relaxed), 2, "local hit still has fresh owner publication");
}

#[test]
fn changing_generation_during_scan_does_not_publish_shared_success() {
    let mut f = fixture("generation", 2);
    let scans = Arc::new(AtomicUsize::new(0)); let owners = Arc::new(AtomicUsize::new(0));
    let generation = shared(&mut f.actors[0]).shared_social_generation.clone();
    let once = Arc::new(AtomicBool::new(true));
    let count = count_hook(scans.clone(), owners.clone());
    let first_hook: ProjectionTestHook = Arc::new(move |phase| {
        count(phase);
        if matches!(phase, ProjectionTestPhase::ScanFinished) && once.swap(false, Ordering::Relaxed) {
            generation.fetch_add(1, Ordering::AcqRel);
        }
    });
    let first = refresh(&mut f.actors[0], first_hook);
    let first_unmarked = shared(&mut f.actors[0]).last_experience_projection.is_none();
    let second = refresh(&mut f.actors[1], count_hook(scans.clone(), owners.clone()));
    let closed = close_fixture(&mut f);
    assert_eq!(closed, 2); assert!(first.is_ok() && second.is_ok());
    assert!(first_unmarked, "changing generation must preserve the original uncompleted-local contract");
    assert_eq!(scans.load(Ordering::Relaxed), 2);
    assert_eq!(owners.load(Ordering::Relaxed), 2);
}

#[test]
fn failed_later_scan_invalidates_older_success_for_an_earlier_entry() {
    let mut f = fixture("failure", 3);
    let mut waiting_actor = f.actors.pop().unwrap();
    let pause = Arc::new(Probe::default());
    let scans = Arc::new(AtomicUsize::new(0)); let owners = Arc::new(AtomicUsize::new(0));
    let count = count_hook(scans.clone(), owners.clone());
    let waiting_hook: ProjectionTestHook = {
        let pause = pause.clone(); let count = count.clone();
        Arc::new(move |phase| {
            if matches!(phase, ProjectionTestPhase::Entry) {
                let mut state = pause.state.lock().unwrap(); state.first_waiting = true; pause.changed.notify_all();
                let (mut state, timeout) = pause.changed.wait_timeout_while(state, Duration::from_secs(5), |s| !s.release).unwrap();
                state.timed_out |= timeout.timed_out();
            }
            count(phase);
        })
    };
    let worker = thread::spawn(move || {
        let result = refresh(&mut waiting_actor, waiting_hook);
        (waiting_actor, result)
    });
    let state = pause.state.lock().unwrap();
    let (state, timeout) = pause.changed.wait_timeout_while(state, Duration::from_secs(5), |s| !s.first_waiting).unwrap();
    let entered = state.first_waiting && !timeout.timed_out(); drop(state);
    let first = refresh(&mut f.actors[0], count.clone());
    let original = replace_stage5(&f, 0, Some("{invalid-peer-after-success".into()));
    let failed = refresh(&mut f.actors[1], count);
    replace_stage5(&f, 0, original);
    { let mut state = pause.state.lock().unwrap(); state.release = true; pause.changed.notify_all(); }
    let joined = worker.join();
    let mut resumed_ok = false;
    if let Ok((actor, result)) = joined { f.actors.push(actor); resumed_ok = result.is_ok(); }
    let closed = close_fixture(&mut f);
    assert_eq!(closed, 3); assert_eq!(f.zone_state.lock().unwrap().players.len(), 0);
    assert!(entered && !pause.state.lock().unwrap().timed_out);
    assert!(first.is_ok() && failed.is_err() && resumed_ok);
    assert_eq!(scans.load(Ordering::Relaxed), 3, "failed miss must erase the older reusable completion");
    assert_eq!(owners.load(Ordering::Relaxed), 3);
}
