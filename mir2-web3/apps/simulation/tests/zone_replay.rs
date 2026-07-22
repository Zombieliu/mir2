use std::process::Command;

use mir2_simulation::{
    gate5_demo_scenario, run_zone_replay_scenario, ZoneInput, ZoneReplayCommand, ZoneReplayEngine,
    ZoneReplayReport,
};

#[test]
fn replay_rejects_missing_duplicate_and_regressed_inputs() {
    let scenario = gate5_demo_scenario(2);
    let mut engine = ZoneReplayEngine::new(scenario.zone_key.clone(), scenario.epoch)
        .expect("engine should initialize");

    engine
        .apply(scenario.inputs[0].clone())
        .expect("first input should apply");
    let duplicate = engine
        .apply(scenario.inputs[0].clone())
        .expect_err("duplicate input must be rejected");
    assert!(duplicate.contains("expected 1, got 0"));

    let gap = engine
        .apply(scenario.inputs[2].clone())
        .expect_err("sequence gap must be rejected");
    assert!(gap.contains("expected 1, got 2"));

    engine
        .apply(scenario.inputs[1].clone())
        .expect("second input should apply");
    engine
        .apply(scenario.inputs[2].clone())
        .expect("third input should apply");
    let regressed = ZoneInput {
        zone_id: scenario.inputs[3].zone_id.clone(),
        epoch: scenario.epoch,
        sequence: 3,
        logical_time_ms: scenario.inputs[2].logical_time_ms.saturating_sub(1),
        command: ZoneReplayCommand::Tick,
    };
    let error = engine
        .apply(regressed)
        .expect_err("logical time regression must be rejected");
    assert!(error.contains("logical time regressed"));
}

#[test]
fn one_hundred_replays_produce_the_same_commitments() {
    let scenario = gate5_demo_scenario(128);
    let expected = run_zone_replay_scenario(scenario.clone())
        .expect("baseline replay should succeed")
        .1;
    for run in 1..100 {
        let actual = run_zone_replay_scenario(scenario.clone())
            .unwrap_or_else(|error| panic!("replay {run} failed: {error}"))
            .1;
        assert_eq!(actual.state_root, expected.state_root, "run {run}");
        assert_eq!(
            actual.checkpoint_hash, expected.checkpoint_hash,
            "run {run}"
        );
        assert_eq!(actual.outbound_count, expected.outbound_count, "run {run}");
    }
}

#[test]
fn ten_thousand_tick_checkpoint_restore_matches_uninterrupted_replay() {
    let scenario = gate5_demo_scenario(10_000);
    let split_at = scenario.inputs.len() / 2;
    let mut first_half = ZoneReplayEngine::new(scenario.zone_key.clone(), scenario.epoch)
        .expect("engine should initialize");
    first_half
        .apply_all(scenario.inputs[..split_at].iter().cloned())
        .expect("first half should apply");
    let checkpoint = first_half
        .checkpoint_bytes()
        .expect("checkpoint should serialize");
    let mut restored = ZoneReplayEngine::restore(&checkpoint).expect("checkpoint should restore");
    let restored_report = restored
        .apply_all(scenario.inputs[split_at..].iter().cloned())
        .expect("restored replay should finish");

    let uninterrupted = run_zone_replay_scenario(scenario)
        .expect("uninterrupted replay should finish")
        .1;
    assert_eq!(restored_report.tick_count, 10_000);
    assert_eq!(restored_report, uninterrupted);
}

#[test]
fn tampered_checkpoint_is_rejected() {
    let scenario = gate5_demo_scenario(32);
    let (engine, _) = run_zone_replay_scenario(scenario).expect("replay should succeed");
    let checkpoint = engine
        .checkpoint_bytes()
        .expect("checkpoint should serialize");
    let mut value: serde_json::Value =
        serde_json::from_slice(&checkpoint).expect("checkpoint should be json");
    value["state_root"] = serde_json::Value::String("00".repeat(32));
    let tampered = serde_json::to_vec(&value).expect("tampered checkpoint should encode");
    let error = ZoneReplayEngine::restore(&tampered)
        .err()
        .expect("tampered checkpoint must be rejected");
    assert!(error.contains("checkpoint replay commitment mismatch"));
}

#[test]
fn independent_processes_match_for_ten_thousand_ticks() {
    let first = run_demo_process(10_000);
    let second = run_demo_process(10_000);
    assert_eq!(
        first, second,
        "independent processes must be byte-identical"
    );

    let report: ZoneReplayReport =
        serde_json::from_slice(&first).expect("binary output should be a replay report");
    assert_eq!(report.tick_count, 10_000);
    assert!(report.applied_inputs > 10_000);
    assert_eq!(report.state_root.len(), 64);
    assert_eq!(report.checkpoint_hash.len(), 64);
}

fn run_demo_process(tick_count: usize) -> Vec<u8> {
    let output = Command::new(env!("CARGO_BIN_EXE_zone_replay"))
        .args(["demo", &tick_count.to_string()])
        .output()
        .expect("zone replay process should start");
    assert!(
        output.status.success(),
        "zone replay process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
