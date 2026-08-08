use std::process::Command;

use mir2_simulation::{
    gate5_demo_scenario, run_zone_replay_scenario, ZoneInput, ZoneReplayCommand, ZoneReplayEngine,
    ZoneReplayReport, ZoneReplicaCheckpoint, ZoneStandbyReplica,
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

#[test]
fn standby_accepts_monotonic_checkpoints_and_rejects_stale_or_conflicting_state() {
    let scenario = gate5_demo_scenario(64);
    let zone_id = scenario.inputs[0].zone_id.clone();
    let mut engine = ZoneReplayEngine::new(scenario.zone_key.clone(), scenario.epoch)
        .expect("engine should initialize");
    engine
        .apply_all(scenario.inputs[..16].iter().cloned())
        .expect("first replica slice should apply");
    let first = ZoneReplicaCheckpoint::capture(&engine, "active-a", scenario.epoch)
        .expect("first checkpoint should capture");
    let mut standby = ZoneStandbyReplica::new(zone_id).expect("standby should initialize");
    assert!(standby
        .accept(first.clone())
        .expect("first checkpoint should replicate"));
    assert!(!standby
        .accept(first.clone())
        .expect("duplicate checkpoint should be idempotent"));

    engine
        .apply_all(scenario.inputs[16..32].iter().cloned())
        .expect("second replica slice should apply");
    let second = ZoneReplicaCheckpoint::capture(&engine, "active-a", scenario.epoch)
        .expect("second checkpoint should capture");
    assert!(standby
        .accept(second)
        .expect("newer checkpoint should replicate"));
    assert!(standby
        .accept(first)
        .expect_err("older checkpoint must be rejected")
        .contains("stale zone replica sequence"));
}

#[test]
fn standby_rejects_corrupted_replica_checkpoint_before_install() {
    let scenario = gate5_demo_scenario(8);
    let (engine, report) = run_zone_replay_scenario(scenario).expect("replay should succeed");
    let mut checkpoint = ZoneReplicaCheckpoint::capture(&engine, "active-a", report.epoch)
        .expect("checkpoint should capture");
    checkpoint.checkpoint_bytes[0] ^= 0x01;
    let mut standby = ZoneStandbyReplica::new(report.zone_id).expect("standby should initialize");
    assert!(standby
        .accept(checkpoint)
        .expect_err("corrupted checkpoint must be rejected")
        .contains("checksum mismatch"));
    assert!(standby.report().is_none());
}

#[test]
fn promoted_standby_rebases_fencing_epoch_and_continues_deterministically() {
    let scenario = gate5_demo_scenario(64);
    let split_at = 24;
    let mut active = ZoneReplayEngine::new(scenario.zone_key.clone(), scenario.epoch)
        .expect("active should initialize");
    active
        .apply_all(scenario.inputs[..split_at].iter().cloned())
        .expect("active prefix should apply");
    let checkpoint = ZoneReplicaCheckpoint::capture(&active, "active-a", scenario.epoch)
        .expect("checkpoint should capture");
    let mut standby = ZoneStandbyReplica::new(checkpoint.report.zone_id.clone())
        .expect("standby should initialize");
    standby
        .accept(checkpoint.clone())
        .expect("checkpoint should replicate");

    let promoted_epoch = scenario.epoch + 1;
    let mut promoted = standby
        .promote(promoted_epoch)
        .expect("standby should promote with a newer fence");
    let mut expected = checkpoint
        .verify()
        .expect("checkpoint should restore")
        .rebase_epoch(promoted_epoch)
        .expect("expected engine should rebase");
    let continuation = scenario.inputs[split_at..]
        .iter()
        .cloned()
        .enumerate()
        .map(|(sequence, mut input)| {
            input.epoch = promoted_epoch;
            input.sequence = sequence as u64;
            input
        })
        .collect::<Vec<_>>();

    let promoted_report = promoted
        .apply_all(continuation.clone())
        .expect("promoted continuation should apply");
    let expected_report = expected
        .apply_all(continuation)
        .expect("expected continuation should apply");
    assert_eq!(promoted_report, expected_report);
    assert_eq!(promoted_report.epoch, promoted_epoch);
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
