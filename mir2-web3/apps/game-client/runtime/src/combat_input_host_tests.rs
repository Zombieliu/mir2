use mir2_client_bevy::combat_input::{
    CombatCommand, CombatController, CombatEdge, CombatOutput, CombatSnapshot,
};
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
