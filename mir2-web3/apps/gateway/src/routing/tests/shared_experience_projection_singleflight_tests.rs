use super::*;
use super::super::shared_experience_profiles::{
    owner_profile_publications_for_test, set_projection_test_hook, ProjectionTestHook, ProjectionTestPhase,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;

#[derive(Default)]
struct ProbeState { entries: usize, owners: usize, scans: usize, first_waiting: bool, release: bool, timed_out: bool }
#[derive(Default)]
struct Probe { state: Mutex<ProbeState>, changed: Condvar }

impl Probe {
    fn hook(self: &Arc<Self>) -> ProjectionTestHook {
        let this = self.clone();
        Arc::new(move |phase| {
            let mut state = this.state.lock().unwrap();
            match phase {
                ProjectionTestPhase::Entry => {
                    state.entries += 1;
                    this.changed.notify_all();
                    let (next, timeout) = this.changed.wait_timeout_while(state, Duration::from_secs(5), |s| s.entries < 2 && !s.timed_out).unwrap();
                    state = next;
                    if timeout.timed_out() { state.timed_out = true; this.changed.notify_all(); }
                }
                ProjectionTestPhase::OwnerPublished => { state.owners += 1; this.changed.notify_all(); }
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

#[test]
fn overlapping_projection_waits_for_complete_scan_but_publishes_each_owner() {
    let config = GatewayConfig::default();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let zone = ZoneId::new("projection-overlap-owned");
    let mut actors = Vec::new();
    for ordinal in 0..2 {
        let mut runtime = factory.create_runtime(config.clone(), &zone);
        start_new_runtime_handle(&mut runtime, &format!("proj{ordinal}"), &format!("ProjNative{ordinal}"));
        actors.push(runtime);
    }
    let resources = factory.resources_for_zone(&zone);
    assert_eq!(resources.zone_state.lock().unwrap().players.len(), 2);
    let shared = actors[0].as_any_mut().downcast_mut::<SharedInProcessZoneSessionRuntime>().unwrap();
    // A real shared generation invalidates both existing per-runtime caches.
    shared.shared_mentors.lock().unwrap().changed();
    let probe = Arc::new(Probe::default());
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
    assert_eq!(scans, 1, "overlapping same-authority/same-domain callers must share one completed projection");
    assert!(!returned_before_completion, "waiter must not return before the first complete projection");
}
