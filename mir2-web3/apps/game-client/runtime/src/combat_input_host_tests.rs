use mir2_client_bevy::combat_input::{
    attack_request_interval_ms, CombatAction, CombatActor, CombatCommand, CombatController,
    CombatEdge, CombatIdentity, CombatOutput, CombatSnapshot, CrystalWorldClickTarget,
};
use mir2_client_bevy::entities::EntityKind;
use super::{ingest_combat_snapshot, CombatHostClock};
fn host_clock_snapshot() -> CombatSnapshot {
    let player = CombatActor {
        object_id: "3".into(), kind: EntityKind::SelfPlayer, x: 10, y: 10,
        direction: Some("up".into()), dead: false, master_object_id: 0, ai: 0,
    };
    let target = CombatActor { object_id: "4".into(), kind: EntityKind::Monster,
        x: 11, ..player.clone() };
    CombatSnapshot {
        identity: CombatIdentity { controller_run: 100, connection_generation: 1,
            session_generation: 2, owner_revision: 0, player_object_id: 3 },
        revision: 1, model_revision: 1, map: "D001".into(), enabled: true, now_ms: 1015,
        player: player.clone(), hp: 10, max_hp: 10, mp: 10, level: 1, attack_speed: 0,
        learned: vec![], actors: vec![player, target], timing: vec![],
        click_targets: vec![CrystalWorldClickTarget { kind: EntityKind::Monster,
            object_id: 4, x: 11, y: 10, dead: Some(false), ai: Some(0), harvestable: Some(false) }],
        selected_object_id: Some(4), force_attack: false, hovered_object_id: Some("4".into()), cursor: None,
        class: Some("Warrior".into()), has_class_weapon: Some(true), riding_mount: Some(false),
        fishing: Some(false), dazed: Some(false), motion_remaining_ms: 0, movement_ready: true,
        planning_position: None, neighbours: vec![], spell_lock_key: "None".into(),
        pointer_spell_lock: false, bindings: None,
    }
}
fn select_once(state: &mut CombatController, snapshot: &CombatSnapshot, now: u64) -> CombatOutput {
    let (handled, outputs) = state.edge(CombatEdge { identity: snapshot.identity,
        model_revision: snapshot.model_revision, map: snapshot.map.clone(), sequence: 1,
        action: CombatAction::Select { object_id: 4 } }, now);
    assert!(handled);
    assert_eq!(outputs.len(), 1);
    outputs.into_iter().next().unwrap()
}
#[test]
fn combat_host_interframe_snapshot_preserves_single_select_attack_cadence() {
    let mut state = CombatController::default();
    let mut clock = CombatHostClock::default();
    let mut snapshot = host_clock_snapshot();
    assert!(ingest_combat_snapshot(&mut state, &mut clock, snapshot.clone(), 1000.));
    let CombatOutput::Wire { proof: first } = select_once(&mut state, &snapshot, clock.now(1000.))
        else { panic!("first selection must attack") };
    assert!(state.tick(clock.now(1016.), 0).is_empty());
    assert_eq!(clock.now(1016.), 1031);
    // The ordinary JS sink/facts refresh can replace the model between frames.
    // Its raw time advances, but is lower than the last extrapolated host tick.
    snapshot.revision = 2;
    snapshot.model_revision = 2;
    snapshot.now_ms = 1020;
    assert!(ingest_combat_snapshot(&mut state, &mut clock, snapshot.clone(), 1016.));
    assert!(state.tick(clock.now(1024.), 0).is_empty());
    assert_eq!(state.snapshot.as_ref().unwrap().model_revision, 2);
    let deadline = 1015 + attack_request_interval_ms(1, 0);
    let deadline_frame = 1016. + (deadline - 1031) as f64;
    assert!(state.tick(clock.now(deadline_frame - 1.), 0).is_empty());
    let outputs = state.tick(clock.now(deadline_frame), 0);
    assert_eq!(outputs.len(), 1);
    let CombatOutput::Wire { proof: second } = &outputs[0]
        else { panic!("one selection must continue attacking after the original interval") };
    assert_eq!(second.identity, first.identity);
    assert_eq!(second.model_revision, 2);
    assert!(second.sequence > first.sequence);
    assert!(matches!(second.command, CombatCommand::Attack { object_id: 4 }));
}
#[test]
fn combat_host_rejected_snapshots_cannot_reanchor_or_restore_canceled_chase() {
    let mut state = CombatController::default();
    let mut clock = CombatHostClock::default();
    let snapshot = host_clock_snapshot();
    assert!(ingest_combat_snapshot(&mut state, &mut clock, snapshot.clone(), 1000.));
    select_once(&mut state, &snapshot, clock.now(1000.));
    assert!(state.tick(clock.now(1016.), 0).is_empty());
    let before = clock;
    let mut stale = snapshot.clone();
    stale.now_ms = 100_000;
    assert!(!ingest_combat_snapshot(&mut state, &mut clock, stale.clone(), 1016.));
    assert_eq!(clock, before);
    stale.identity.controller_run = 99;
    stale.revision = 2;
    assert!(!ingest_combat_snapshot(&mut state, &mut clock, stale, 1016.));
    assert_eq!(clock, before);
    assert!(state.snapshot.is_some());
    let mut invalid = snapshot.clone();
    invalid.revision = 2;
    invalid.now_ms = 100_000;
    invalid.learned = vec![serde_json::Value::Null];
    assert!(!ingest_combat_snapshot(&mut state, &mut clock, invalid, 1016.));
    assert_eq!(clock, before);
    assert!(state.snapshot.is_none());
    let mut restored = snapshot;
    restored.revision = 3;
    restored.now_ms = 1020;
    assert!(ingest_combat_snapshot(&mut state, &mut clock, restored, 1016.));
    assert!(state.tick(clock.now(5000.), 0).is_empty());
}
#[test]
fn combat_host_clock_limits_still_retire_controller_without_wraparound() {
    let mut state = CombatController::default();
    let mut clock = CombatHostClock::default();
    let mut snapshot = host_clock_snapshot();
    snapshot.now_ms = mir2_client_bevy::skill_page_state::MAX_SAFE_ID;
    assert!(ingest_combat_snapshot(&mut state, &mut clock, snapshot, 1000.));
    assert!(state.tick(clock.now(1016.), 0).is_empty());
    assert!(state.snapshot.is_none());
    let saturated = CombatHostClock { base_ms: u64::MAX - 2, frame_ms: 0. };
    assert_eq!(saturated.now(16.), u64::MAX);
    assert_eq!(saturated.now(u64::MAX as f64), u64::MAX);
}
#[test]
fn combat_wire_rejects_unknown_fields_and_unsafe_identity_shape() {
    assert!(serde_json::from_value::<CombatEdge>(serde_json::json!({"identity":{"controllerRun":1,"connectionGeneration":1,"sessionGeneration":1,"ownerRevision":0,"playerObjectId":3},"modelRevision":1,"map":"D001","sequence":1,"action":{"type":"cancel"},"extra":true})).is_err());
    assert!(serde_json::from_value::<CombatEdge>(serde_json::json!({"identity":{"controllerRun":1,"connectionGeneration":1,"sessionGeneration":1,"ownerRevision":0,"playerObjectId":3,"extra":true},"modelRevision":1,"map":"D001","sequence":1,"action":{"type":"cancel"}})).is_err());
}
#[test]
#[ignore = "Root supplies actual complete Web setter and event captures"]
fn actual_web_combat_snapshot_and_edge_are_strict_and_owned() {
    let snapshot = std::fs::read(
        std::env::var("MIR2_M11_COMBAT_SNAPSHOT_FIXTURE").expect("actual setter fixture required"),
    )
    .unwrap();
    let edge = std::fs::read(
        std::env::var("MIR2_M11_COMBAT_EDGE_FIXTURE").expect("actual edge fixture required"),
    )
    .unwrap();
    let s: CombatSnapshot = serde_json::from_slice(&snapshot).unwrap();
    let e: CombatEdge = serde_json::from_slice(&edge).unwrap();
    assert!(s.validate());
    assert_eq!(s.identity, e.identity);
    assert_eq!(s.model_revision, e.model_revision);
    assert_eq!(s.learned[0]["icon"], 0);
    assert!(s.learned[0]["need2"].is_null());
    assert_eq!(s.learned[0]["magicName"], "火球");
    let mut controller = CombatController::default();
    assert!(controller.ingest(s.clone()));
    let (handled, outputs) = controller.edge(e.clone(), s.now_ms);
    assert!(handled);
    assert_eq!(outputs.len(), 1);
    let CombatOutput::Wire { proof } = outputs[0].clone() else {
        panic!("actual edge must produce wire")
    };
    assert_eq!(proof.slot, Some(1));
    assert_eq!(proof.skill_id, Some(0));
    assert_eq!(proof.spell.as_deref(), Some("FireBall"));
    assert!(matches!(
        proof.command,
        CombatCommand::Magic {
            object_id: 3,
            target_id: 4,
            ..
        }
    ));
    assert!(controller.edge(e, s.now_ms).1.is_empty());
    let mut bad = s.clone();
    bad.revision += 1;
    bad.learned = vec![serde_json::Value::Null];
    assert!(!controller.ingest(bad));
    assert!(controller.snapshot.is_none());
}
