use super::*;
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::ClientPacket;

fn binding(epoch: u64) -> PlayerDropOperationBinding {
    PlayerDropOperationBinding::new(
        "original-nonce".into(),
        "rpc-session".into(),
        "demo".into(),
        0,
        "owner-group".into(),
        "owner-a".into(),
        epoch,
        17,
    )
    .unwrap()
}
fn intent() -> PlayerDropWorldIntent {
    PlayerDropWorldIntent::new(
        binding(3),
        ZoneKey {
            shard_id: "s".into(),
            map_file_name: "0".into(),
            channel_id: 0,
            instance_id: String::new(),
        },
        Point { x: 1, y: 2 },
        100,
        1_800_100,
        PlayerDropOutcome::Gold { amount: 7 },
    )
    .unwrap()
}
fn save() -> CharacterSaveRecord {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.active_character_checkpoint().unwrap()
}

#[test]
fn player_drop_effect_identity_is_epoch_scoped_and_domain_separated() {
    let original = binding(3);
    assert_ne!(original.operation_key(), original.world_effect_id());
    assert_ne!(original.world_effect_id(), binding(4).world_effect_id());
    let decoded: PlayerDropOperationBinding =
        serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    assert_eq!(original.world_effect_id(), decoded.world_effect_id());
    assert_eq!(original.source_rpc_session(), "rpc-session");
}

#[test]
fn player_drop_intent_detects_expiry_placement_payload_and_binding_tampering() {
    let value = serde_json::to_value(intent()).unwrap();
    for (key, replacement) in [
        ("expires_at_unix_ms", serde_json::json!(1_900_100)),
        ("position", serde_json::json!({"x": 9, "y": 2})),
        ("world_effect_id", serde_json::json!("different-effect")),
        ("outcome", serde_json::json!({"kind":"Gold","amount":8})),
    ] {
        let mut changed = value.clone();
        changed[key] = replacement;
        let decoded: PlayerDropWorldIntent = serde_json::from_value(changed).unwrap();
        assert!(decoded.validate_integrity().is_err(), "{key}");
    }
    let decoded: PlayerDropWorldIntent =
        serde_json::from_str(&serde_json::to_string(&intent()).unwrap()).unwrap();
    decoded.validate_integrity().unwrap();
}

#[test]
fn player_drop_complete_operation_and_receipt_reject_rebound_checkpoints() {
    let before = save();
    let mut after = before.clone();
    after.revision += 1;
    after.gold -= 7;
    let operation =
        PlayerDropPreparedOperation::new(intent(), before.clone(), before, after).unwrap();
    let receipt = PlayerDropCommitReceipt::new(
        operation.binding().operation_key(),
        operation.intent().world_effect_id().into(),
        operation.operation_digest().into(),
        operation.intent().intent_digest().into(),
        operation.checkpoint().revision,
        PlayerDropEffectState::Live,
    );
    receipt.validate_operation(&operation).unwrap();
    let mut changed = serde_json::to_value(&operation).unwrap();
    changed["checkpoint"]["gold"] = serde_json::json!(12345);
    let changed: PlayerDropPreparedOperation = serde_json::from_value(changed).unwrap();
    assert!(changed.validate_integrity().is_err());
    assert!(receipt.validate_operation(&changed).is_err());
}

#[test]
fn player_drop_ordinary_uid_successor_never_wraps() {
    assert_eq!(player_drop_next_ordinary_item_uid(0).unwrap(), 1);
    assert!(player_drop_next_ordinary_item_uid(u64::MAX).is_err());
}

fn journal() -> PlayerDropJournalResult {
    let before = save();
    let mut after = before.clone();
    after.revision += 1;
    after.gold -= 7;
    let intent = PlayerDropWorldIntent::new(
        binding(3),
        ZoneKey {
            shard_id: "s".into(),
            map_file_name: before.map_file_name.clone(),
            channel_id: 0,
            instance_id: String::new(),
        },
        before.position.clone(),
        100,
        1_800_100,
        PlayerDropOutcome::Gold { amount: 7 },
    )
    .unwrap();
    let reservation = serde_json::from_value(serde_json::json!({
        "zone": intent.zone(), "world_effect_id": intent.world_effect_id(), "session_id": "rpc-session",
        "player_object_id": 1, "origin": before.position, "position": intent.position(),
        "kind": "Gold", "reserved_at_unix_ms": 100,
    })).unwrap();
    let operation =
        PlayerDropPreparedOperation::new(intent, before.clone(), before, after).unwrap();
    let receipt = PlayerDropCommitReceipt::new(
        operation.binding().operation_key(),
        operation.intent().world_effect_id().into(),
        operation.operation_digest().into(),
        operation.intent().intent_digest().into(),
        operation.checkpoint().revision,
        PlayerDropEffectState::Live,
    );
    PlayerDropJournalResult {
        operation,
        receipt,
        request: PlayerDropRequest::Gold { amount: 7 },
        reservation: Some(reservation),
    }
}

#[test]
fn player_drop_journal_binds_complete_receipt_and_original_source_placement() {
    let journal = journal();
    journal.validate_integrity().unwrap();
    let value = serde_json::to_value(&journal).unwrap();
    for (key, replacement) in [
        ("world_effect_id", serde_json::json!("different")),
        ("session_id", serde_json::json!("different-source")),
        ("kind", serde_json::json!("Item")),
        ("reserved_at_unix_ms", serde_json::json!(101)),
        ("position", serde_json::json!({ "x": 1, "y": 2 })),
    ] {
        let mut changed = value.clone();
        changed["reservation"][key] = replacement;
        let decoded: PlayerDropJournalResult = serde_json::from_value(changed).unwrap();
        assert!(decoded.validate_integrity().is_err(), "{key}");
    }
    let mut missing = journal.clone();
    missing.reservation = None;
    assert!(missing.validate_integrity().is_err());
    assert!(serde_json::to_value(missing)
        .unwrap()
        .get("reservation")
        .is_none());
}

#[test]
fn player_drop_journal_requires_and_binds_exact_recorded_request() {
    let mut result = journal();
    result.validate_integrity().unwrap();
    result.request = PlayerDropRequest::Gold { amount: 8 };
    assert!(result.validate_integrity().is_err());
    let mut missing = serde_json::to_value(journal()).unwrap();
    missing.as_object_mut().unwrap().remove("request");
    assert!(serde_json::from_value::<PlayerDropJournalResult>(missing).is_err());
}
