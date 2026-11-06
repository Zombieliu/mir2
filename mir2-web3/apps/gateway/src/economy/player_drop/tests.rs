use super::*;
use mir2_simulation::{
    GroundDropSnapshot, PlayerDropEffectIdentity, PlayerDropWorldIntent, ZoneKey,
};

pub(super) fn fixture() -> PlayerDropPreparedOperation {
    let runtime = crate::economy::tests::start_test_runtime("drop-source", "DropSource").unwrap();
    let identity = runtime.active_identity().unwrap();
    let mut before = runtime.active_character_checkpoint().unwrap();
    before.gold = 100;
    let mut checkpoint = before.clone();
    checkpoint.gold = 75;
    checkpoint.revision += 1;
    let binding = PlayerDropOperationBinding::new(
        "operation-1".into(),
        "rpc-session-1".into(),
        identity.account_id,
        identity.character_index,
        "zone-1".into(),
        "owner-1".into(),
        7,
        42,
    )
    .unwrap();
    let zone = ZoneKey {
        shard_id: "shard-1".into(),
        map_file_name: before.map_file_name.clone(),
        channel_id: 1,
        instance_id: "".into(),
    };
    let intent = PlayerDropWorldIntent::new(
        binding,
        zone,
        mir2_protocol::Point { x: 10, y: 20 },
        100,
        10000,
        PlayerDropOutcome::Gold { amount: 25 },
    )
    .unwrap();
    fn canonical(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => serde_json::Value::Object(
                fields
                    .into_iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .map(|(key, value)| (key, canonical(value)))
                    .collect(),
            ),
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(canonical).collect())
            }
            value => value,
        }
    }
    let bytes = serde_json::to_vec(&canonical(
        serde_json::to_value((&intent, &before, &before, &checkpoint)).unwrap(),
    ))
    .unwrap();
    let mut hash = Sha256::new();
    hash.update(b"mir2/player-drop/prepared-source/v1\0");
    hash.update(bytes);
    serde_json::from_value(serde_json::json!({"intent":intent,"before":before,"runtime_before":before,"checkpoint":checkpoint,"operation_digest":hex_lower(&hash.finalize())})).unwrap()
}

pub(super) fn pickup(
    operation: &PlayerDropPreparedOperation,
    account: &str,
    object_id: u32,
) -> SharedAccountInventoryCommandEnvelope {
    let effect:PlayerDropEffectIdentity=serde_json::from_value(serde_json::json!({"world_effect_id":operation.intent().world_effect_id(),
        "intent_digest":operation.intent().intent_digest(),"expires_at_unix_ms":operation.intent().expires_at_unix_ms()})).unwrap();
    SharedAccountInventoryCommandEnvelope {
        identity: ActiveSessionIdentity {
            account_id: account.into(),
            character_index: 1,
            character_name: account.into(),
        },
        command: SharedAccountInventoryCommand::PlayerDropClaimPickup {
            drop: GroundDropSnapshot {
                object_id,
                name: "Gold".into(),
                name_colour_argb: 0,
                icon: 0,
                x: 10,
                y: 20,
                quantity: 25,
                source_monster: String::new(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 25 },
            },
            claim_idempotency_key: format!("player-drop-pickup:{}", effect.world_effect_id()),
            effect,
        },
    }
}

pub(super) fn context() -> SharedAccountInventoryExecutionContext {
    SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new("zone-1"),
        fencing_generation: 7,
        source_sequence: 43,
        created_at_ms: 200,
        external_commit_authorized: true,
    }
}

#[test]
fn player_drop_source_uses_complete_source_delta_and_preserves_historical_kill_projection() {
    let operation = fixture();
    operation.validate_integrity().unwrap();
    let (legs, expected) = source_delta(&operation).unwrap();
    assert_eq!(legs.len(), 1);
    assert_eq!(legs[0].delta, -25);
    assert_eq!(expected[&legs[0].balance], 100);
    let projection = PlayerDropSourceProjection::new(&operation).unwrap();
    let envelope = source_envelope(&operation, legs);
    projection.validate_envelope(&envelope).unwrap();
    let mut changed = envelope.clone();
    changed.legs[0].delta = -26;
    assert!(projection.validate_envelope(&changed).is_err());
    let mut value = serde_json::to_value(&projection).unwrap();
    value["operation"]["checkpoint"]["gold"] = serde_json::json!(999);
    let changed: PlayerDropSourceProjection = serde_json::from_value(value).unwrap();
    assert!(changed.digest().is_err());
    let receipt = EconomyTransactionReceipt {
        idempotency_key: envelope.idempotency_key.clone(),
        event_id: envelope.event_id().unwrap(),
        transaction_kind: envelope.transaction_kind,
        committed_at_ms: 200,
        balances_after: BTreeMap::from([(
            balance_receipt_key(&envelope.legs[0].balance).unwrap(),
            75,
        )]),
        duplicate: false,
        settled_elsewhere: false,
        source_checkpoint_digest: Some(projection.digest().unwrap()),
    };
    let event = EconomyOutboxEvent {
        event_id: receipt.event_id.clone(),
        idempotency_key: receipt.idempotency_key.clone(),
        envelope,
        receipt_digest: receipt_integrity_digest(&receipt).unwrap(),
        source_checkpoint: None,
        player_drop_source: Some(projection),
    };
    validate_stored_economy_transaction(&event, &receipt).unwrap();
    let mut corrupted = receipt.clone();
    corrupted.source_checkpoint_digest = Some("0".repeat(64));
    assert!(validate_stored_economy_transaction(&event, &corrupted).is_err());
    let mut legacy = event;
    legacy.player_drop_source = None;
    legacy.source_checkpoint = None;
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("playerDropSource")
        .is_none());
}

#[test]
fn player_drop_pickup_logical_identity_survives_presentation_change_and_preserves_recipient() {
    let operation = fixture();
    let first =
        economy_transaction_for_command(&context(), &pickup(&operation, "Alice", 1), None).unwrap();
    let new_object =
        economy_transaction_for_command(&context(), &pickup(&operation, "Alice", 900), None)
            .unwrap();
    assert_eq!(first.idempotency_key, new_object.idempotency_key);
    assert_eq!(
        duplicate_pickup_effect(&first, &new_object).unwrap(),
        Some(DuplicateBusinessEffect::Exact)
    );
    let other =
        economy_transaction_for_command(&context(), &pickup(&operation, "Bob", 901), None).unwrap();
    assert_eq!(
        duplicate_pickup_effect(&first, &other).unwrap(),
        Some(DuplicateBusinessEffect::SettledElsewhere)
    );
    let encoded = first.metadata.get("groundDropProjectionV1").unwrap();
    let intent: GroundDropProjectionIntent = serde_json::from_str(encoded).unwrap();
    assert_eq!(intent.identity.account_id, "Alice");
    validate_pickup_intent(&operation, &first, &intent).unwrap();
}

#[test]
fn player_drop_pickup_rejects_changed_amount_placement_expiry_and_missing_binding() {
    let operation = fixture();
    let original =
        economy_transaction_for_command(&context(), &pickup(&operation, "Alice", 1), None).unwrap();
    for mutation in ["amount", "placement", "expiry", "effect"] {
        let mut command = pickup(&operation, "Alice", 2);
        let SharedAccountInventoryCommand::PlayerDropClaimPickup { drop, effect, .. } =
            &mut command.command
        else {
            unreachable!()
        };
        match mutation {
            "amount" => {
                drop.quantity = 26;
                drop.loot = GroundDropLootSnapshot::Gold { amount: 26 };
            }
            "placement" => drop.x += 1,
            "expiry" => {
                let mut value = serde_json::to_value(&effect).unwrap();
                value["expires_at_unix_ms"] = serde_json::json!(11000);
                *effect = serde_json::from_value(value).unwrap();
            }
            "effect" => {
                let mut value = serde_json::to_value(&effect).unwrap();
                value["intent_digest"] = serde_json::json!("0".repeat(64));
                *effect = serde_json::from_value(value).unwrap();
            }
            _ => unreachable!(),
        }
        let changed = economy_transaction_for_command(&context(), &command, None).unwrap();
        assert!(
            duplicate_pickup_effect(&original, &changed).is_err(),
            "{mutation}"
        );
        let projection: GroundDropProjectionIntent =
            serde_json::from_str(changed.metadata.get("groundDropProjectionV1").unwrap()).unwrap();
        assert!(
            validate_pickup_intent(&operation, &changed, &projection).is_err(),
            "{mutation}"
        );
    }
    let mut command = pickup(&operation, "Alice", 2);
    let SharedAccountInventoryCommand::PlayerDropClaimPickup {
        claim_idempotency_key,
        ..
    } = &mut command.command
    else {
        unreachable!()
    };
    *claim_idempotency_key = "old-presentation-key".into();
    assert!(economy_transaction_for_command(&context(), &command, None).is_none());
}
