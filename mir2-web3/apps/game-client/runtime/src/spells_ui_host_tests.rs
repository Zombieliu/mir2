use super::*;
fn value() -> serde_json::Value {
    serde_json::json!({"requestRun":7,"connectionGeneration":1,"sessionGeneration":2,"ownerRevision":0,"hudGeneration":3,"playerObjectId":4,"revision":1,"modelRevision":1,"presentationRevision":1,"open":true,"inputEnabled":true,"presentation":{"logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":false},"player":PlayerStats::default(),"learned":[{"spell":"FireBall","magicName":"火球","icon":0,"need1":0,"need2":null,"hotkey":1}],"receipts":[],"timing":[],"nowMs":10,"iconMetadata":{"version":3,"frames":[{"index":0,"x":0,"y":0,"width":36,"height":34},{"index":1,"x":0,"y":0,"width":36,"height":34}]}})
}
#[test]
fn m10_spells_host_exact_raw_zero_null_and_sparse_metadata() {
    let s = SpellsSnapshot::parse(&value().to_string()).unwrap();
    let (m, _) = s.models().unwrap();
    assert_eq!(m.skills[0].name, "火球");
    assert_eq!(m.binding_for(0).icon, Some(0));
    assert_eq!(m.binding_for(0).need1, Some(0));
    assert_eq!(m.binding_for(0).need2, None);
    assert_eq!(m.bindings[0].cast_sequence, 0);
    let mut v = value();
    v["invented"] = true.into();
    assert!(SpellsSnapshot::parse(&v.to_string()).is_err());
}
#[test]
fn m10_pending_mailbox_highwater_preserves_complete_ack_before_ingest() {
    let mut a = value();
    let ack = serde_json::json!({"requestId":7*(1_u64<<22)+1,"spell":"FireBall","key":16,"oldKey":1,"accepted":true});
    a["receipts"] =
        serde_json::json!([{"serial":1,"learned":a["learned"].clone(),"skillKeyAck":ack}]);
    let mut b = value();
    b["revision"] = 2.into();
    let mut mailbox = SpellsMailbox::default();
    assert!(mailbox.accept(SpellsSnapshot::parse(&a.to_string()).unwrap()));
    assert!(mailbox.accept(SpellsSnapshot::parse(&b.to_string()).unwrap()));
    assert!(!mailbox.accept(SpellsSnapshot::parse(&a.to_string()).unwrap()));
    let snapshot = mailbox.take().unwrap();
    assert_eq!(snapshot.revision, 2);
    assert_eq!(snapshot.receipts.len(), 1);
    assert_eq!(
        snapshot.receipts[0].learned[0]["need2"],
        serde_json::Value::Null
    );
    assert_eq!(
        snapshot.receipts[0].skill_key_ack.request_id,
        7 * (1_u64 << 22) + 1
    );
}
#[test]
fn m10_spells_host_rejects_wrong_namespace_timing_and_compact_touch() {
    let mut v = value();
    v["receipts"] = serde_json::json!([{"serial":1,"learned":v["learned"].clone(),"skillKeyAck":{"requestId":8*(1_u64<<22)+1,"spell":"FireBall","key":16,"oldKey":1,"accepted":true}}]);
    assert!(SpellsSnapshot::parse(&v.to_string()).is_err());
    let mut v = value();
    v["timing"] =
        serde_json::json!([{"spell":"FireBall","sequence":1,"observedAtMs":11,"delayMs":2200}]);
    assert!(SpellsSnapshot::parse(&v.to_string()).is_err());
    let mut v = value();
    v["presentation"]["touch"] = true.into();
    v["presentation"]["stageCssScale"] = (600_f64 / 1024.).into();
    assert!(SpellsSnapshot::parse(&v.to_string()).is_err());
}
#[test]
#[ignore = "Root supplies an actual executed JS Host setter capture after Source application"]
fn m10_actual_js_spells_setter_capture_matches_strict_rust_snapshot() {
    let path =
        std::env::var("MIR2_M10_SPELLS_SNAPSHOT_FIXTURE").expect("explicit Root fixture input");
    let s = SpellsSnapshot::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(s.identity.valid());
    assert!(!s.learned.is_empty());
}
