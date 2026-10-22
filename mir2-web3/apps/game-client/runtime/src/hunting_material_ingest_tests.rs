//! Real NativeInbound admission, typed prefix consumption and the existing
//! exclusive owner-consumer chain. No Windows socket or GUI is simulated here.
use super::*;
use crate::hunting_material_economy::{NativeHuntingMaterialGate, NativeHuntingMaterialSource};
use bevy::{
    ecs::system::RunSystemOnce,
    prelude::{DetectChanges, World},
};
use mir2_client_bevy::{
    hunting_material::{HuntingMaterialModel, HuntingMaterialPhase},
    inventory::InventoryModel,
    pending_operations::{AuthoritativeModelRevisions, PendingOperations},
    storage::StorageModel,
};
use serde_json::json;

fn material() -> (NativeHuntingMaterialGate, NativeInboundMessage) {
    let (gate, quote, receipt, projection) = crate::hunting_material_economy::tests::fixture(true);
    let bundle = gate.prepare(1, &quote, &receipt, projection).unwrap();
    (gate, NativeInboundMessage::HuntingMaterialBundle(bundle))
}

fn local() -> NativeInboundBuffer {
    NativeInboundBuffer {
        active: true,
        ..Default::default()
    }
}

fn runtime_world(state: NativeInboundBuffer) -> World {
    let mut world = World::new();
    world.insert_resource(NativeInbound {
        buffer: Arc::new(Mutex::new(state)),
    });
    world.insert_resource(InventoryModel::default());
    world.insert_resource(StorageModel::default());
    world.insert_resource(AuthoritativeModelRevisions::default());
    world.insert_resource(PendingOperations::default());
    world
}

fn inventory(gold: u32) -> String {
    serde_json::to_string(&InventoryModel {
        gold,
        ..Default::default()
    })
    .unwrap()
}

fn game_receipt(id: &str) -> String {
    json!({"protocol":"nativeGameShopReceiptV1","requestId":id,"success":false,
        "gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"})
    .to_string()
}

fn typed_game_receipt() -> mir2_client_bevy::game_shop::GameShopReceipt {
    serde_json::from_str(&game_receipt("material-reset")).unwrap()
}

#[test]
fn native_hunting_material_real_producer_admission_and_typed_fifo_apply_before_completion() {
    let _serial = native_queue_test_guard();
    let inbound = NativeInbound::new();
    let (gate, message) = material();
    let NativeInboundMessage::HuntingMaterialBundle(bundle) = message else {
        unreachable!()
    };
    assert!(push_native_inventory_model(inventory(77)));
    assert!(push_native_hunting_material_bundle(bundle));
    assert!(push_native_inventory_model(inventory(99)));
    assert!(
        gate.try_recv_applied().is_none(),
        "real enqueue is not Applied"
    );
    let mut world = World::new();
    world.insert_resource(inbound);
    world.insert_resource(InventoryModel::default());
    world.insert_resource(StorageModel::default());
    world.insert_resource(AuthoritativeModelRevisions::default());
    world.insert_resource(PendingOperations::default());
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(
        gate.try_recv_applied().is_none(),
        "undrained ordinary prefix prevents application"
    );
    world
        .run_system_once(crate::ingest_pending_inventory_model)
        .unwrap();
    assert_eq!(world.resource::<InventoryModel>().gold, 77);
    assert!(gate.try_recv_applied().is_none());
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert_eq!(
        world.resource::<InventoryModel>().gold,
        90,
        "complete material owner committed"
    );
    let applied = gate.try_recv_applied().unwrap();
    assert_eq!(
        world.resource::<NativeHuntingMaterialSource>().applied(),
        &applied
    );
    assert_eq!(
        gate.with_current_applied(&applied, || applied.request_id()),
        Some(1)
    );
    world
        .run_system_once(crate::ingest_pending_inventory_model)
        .unwrap();
    assert_eq!(
        world.resource::<InventoryModel>().gold,
        99,
        "later ordinary model waits for complete application"
    );
    drop(world);
    let (_, message) = material();
    let NativeInboundMessage::HuntingMaterialBundle(bundle) = message else {
        unreachable!()
    };
    assert!(
        !push_native_hunting_material_bundle(bundle),
        "dropped actual receiver cannot admit a producer"
    );
}

#[test]
fn native_hunting_material_three_lane_consumer_preserves_mixed_prefix_and_retires_applied_source() {
    let (purchase, witness, purchase_projection) = crate::npc_purchase_economy::tests::fixture();
    let purchase_bundle = purchase
        .prepare(witness, purchase_projection.clone())
        .unwrap();
    let (material_gate, material_bundle) = material();
    let repair = crate::hunting_repair_economy::NativeHuntingRepairGate::new(
        crate::hunting_repair_economy::NativeHuntingRepairEpoch {
            run: 1,
            connection: 2,
            procedure: 3,
            owner_epoch: 4,
            scene_epoch: 5,
            cancellation: 0,
            actor: 3,
            map: 0,
        },
    )
    .unwrap();
    let repair_bundle = repair
        .prepare(1, &"12".repeat(32), purchase_projection)
        .unwrap();
    let mut state = local();
    assert!(state.enqueue(NativeInboundMessage::NpcEconomyBundle(purchase_bundle)));
    assert!(state.enqueue(NativeInboundMessage::InventoryModel(inventory(77))));
    assert!(state.enqueue(material_bundle));
    assert!(state.enqueue(NativeInboundMessage::InventoryModel(inventory(88))));
    assert!(state.enqueue(NativeInboundMessage::HuntingRepairBundle(repair_bundle)));
    assert!(state.enqueue(NativeInboundMessage::InventoryModel(inventory(99))));
    let mut world = runtime_world(state);
    world
        .run_system_once(crate::ingest_pending_inventory_model)
        .unwrap();
    assert_eq!(world.resource::<InventoryModel>().gold, 0);
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(purchase.try_recv_applied().is_some());
    assert!(
        material_gate.epoch().is_some(),
        "later lane admission cannot retire a valid earlier prefix"
    );
    assert!(material_gate.try_recv_applied().is_none() && repair.try_recv_applied().is_none());
    world
        .run_system_once(crate::ingest_pending_inventory_model)
        .unwrap();
    assert_eq!(world.resource::<InventoryModel>().gold, 77);
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    let applied = material_gate.try_recv_applied().unwrap();
    assert!(!world.contains_resource::<crate::npc_purchase_economy::NativeNpcEconomySource>());
    world
        .run_system_once(crate::ingest_pending_inventory_model)
        .unwrap();
    assert_eq!(world.resource::<InventoryModel>().gold, 88);
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(repair.try_recv_applied().is_some());
    assert!(material_gate
        .with_current_applied(&applied, || ())
        .is_none());
    assert!(!world.contains_resource::<NativeHuntingMaterialSource>());
    world
        .run_system_once(crate::ingest_pending_inventory_model)
        .unwrap();
    assert_eq!(world.resource::<InventoryModel>().gold, 99);

    // The reverse direction also clears material custody on an actual purchase
    // commit, rather than relying only on the repair transition above.
    let (material_gate, bundle) = material();
    let mut state = local();
    assert!(state.enqueue(bundle));
    let mut world = runtime_world(state);
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    let old = material_gate.try_recv_applied().unwrap();
    let (new_purchase, witness, projection) = crate::npc_purchase_economy::tests::fixture();
    assert!(world
        .resource::<NativeInbound>()
        .buffer
        .lock()
        .unwrap()
        .enqueue(NativeInboundMessage::NpcEconomyBundle(
            new_purchase.prepare(witness, projection).unwrap()
        )));
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(new_purchase.try_recv_applied().is_some());
    assert!(material_gate.with_current_applied(&old, || ()).is_none());
    assert!(!world.contains_resource::<NativeHuntingMaterialSource>());
}

#[test]
fn native_hunting_material_all_reset_admissions_immediately_retire_queued_and_applied_proofs() {
    for reset in [
        NativeInboundMessage::DataReset,
        NativeInboundMessage::SceneReset,
        NativeInboundMessage::DataResetPreservingExactGameShopReceipt(typed_game_receipt()),
    ] {
        let (gate, bundle) = material();
        let mut state = local();
        assert!(state.enqueue(bundle));
        assert!(state.enqueue(reset.clone()));
        assert!(gate.epoch().is_none());
        assert!(gate.try_recv_applied().is_none());
        assert!(!state.pending.iter().any(is_owner_bundle));

        let (gate, bundle) = material();
        let mut state = local();
        assert!(state.enqueue(bundle));
        let mut world = runtime_world(state);
        crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
        let popped = gate.try_recv_applied().unwrap();
        assert!(world
            .resource::<NativeInbound>()
            .buffer
            .lock()
            .unwrap()
            .enqueue(reset.clone()));
        assert!(
            gate.epoch().is_none(),
            "reset admission retires before any later Bevy consumer runs"
        );
        assert!(gate.with_current_applied(&popped, || ()).is_none());

        // Defense in depth for stale-discard input injected without normal
        // admission: retiring a removed bundle is still required.
        let (gate, bundle) = material();
        let mut state = local();
        state.pending.push_back(bundle);
        state.pending.push_back(reset);
        let inbound = NativeInbound {
            buffer: Arc::new(Mutex::new(state)),
        };
        inbound.discard_stale_data_before_latest_reset();
        assert!(gate.epoch().is_none());
    }
}

#[test]
fn native_hunting_material_exact_byte_capacity_and_pressure_preserve_whole_barrier() {
    let (gate, bundle) = material();
    let mut state = local();
    assert!(state.enqueue(NativeInboundMessage::InventoryModel(inventory(77))));
    let prefix_bytes = state.pending_bytes();
    let prefix_count = state.message_count();
    let bytes = native_message_bytes(&bundle);
    assert!(!state.enqueue_with_limits(bundle.clone(), bytes - 1, usize::MAX));
    assert!(!state.enqueue_with_limits(bundle.clone(), usize::MAX, prefix_bytes + bytes - 1));
    assert_eq!(
        (state.pending_bytes(), state.message_count()),
        (prefix_bytes, prefix_count)
    );
    assert!(gate.try_recv_applied().is_none());
    assert!(state.enqueue_with_limits(bundle, bytes, prefix_bytes + bytes));
    for i in 0..MAX_NATIVE_MESSAGES * 2 {
        let _ = state.enqueue(NativeInboundMessage::InventoryModel(format!(
            "snapshot-{i}"
        )));
        let _ = state.enqueue(NativeInboundMessage::InventoryOperationAck(format!(
            "ack-{i}"
        )));
    }
    assert!(state
        .pending
        .iter()
        .any(|message| matches!(message, NativeInboundMessage::HuntingMaterialBundle(_))));
    let (gate, bundle) = material();
    let mut only = local();
    assert!(only.enqueue(bundle));
    assert!(!only.evict_oldest_non_boundary());
    assert!(!only.evict_oldest_non_ack_non_boundary());
    assert!(!only.evict_oldest_coalescible_snapshot());
    let (retired, late) = material();
    retired.retire();
    assert!(!only.enqueue(late));
    assert!(gate.epoch().is_some());
}

#[test]
fn native_hunting_material_actual_producer_replacement_and_drop_retire_queued_gate() {
    let _serial = native_queue_test_guard();
    let old = NativeInbound::new();
    let (old_gate, bundle) = material();
    let NativeInboundMessage::HuntingMaterialBundle(bundle) = bundle else {
        unreachable!()
    };
    assert!(push_native_hunting_material_bundle(bundle));
    let current = NativeInbound::new();
    assert!(old_gate.epoch().is_none());
    let (new_gate, bundle) = material();
    let NativeInboundMessage::HuntingMaterialBundle(bundle) = bundle else {
        unreachable!()
    };
    assert!(push_native_hunting_material_bundle(bundle));
    drop(old);
    assert!(
        new_gate.epoch().is_some(),
        "dropping a replaced old receiver cannot retire its replacement"
    );
    drop(current);
    assert!(new_gate.epoch().is_none());
}

#[test]
fn native_hunting_material_dedicated_terminals_and_mail_materialization_cannot_overtake() {
    let (gate, bundle) = material();
    let mut state = local();
    assert!(
        state.enqueue(NativeInboundMessage::GameShopReceipt(game_receipt(
            "prefix"
        )))
    );
    assert!(state.enqueue(bundle));
    assert!(state.enqueue(NativeInboundMessage::GameShopReceipt(game_receipt("later"))));
    let mut world = runtime_world(state);
    let mut before = Vec::new();
    world.resource::<NativeInbound>().drain_matching(
        |message| matches!(message, NativeInboundMessage::GameShopReceipt(_)),
        |message| before.push(message),
    );
    assert!(
        matches!(&before[..],[NativeInboundMessage::GameShopReceipt(value)] if value.contains("prefix"))
    );
    assert!(gate.try_recv_applied().is_none());
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(gate.try_recv_applied().is_some());
    let mut after = Vec::new();
    world.resource::<NativeInbound>().drain_matching(
        |message| matches!(message, NativeInboundMessage::GameShopReceipt(_)),
        |message| after.push(message),
    );
    assert!(
        matches!(&after[..],[NativeInboundMessage::GameShopReceipt(value)] if value.contains("later"))
    );

    let (_, bundle) = material();
    let mut state = local();
    let epoch = MailServiceStreamEpoch {
        run: 1,
        connection: 1,
    };
    assert!(
        state.enqueue(NativeInboundMessage::MailServiceStreamStarted(
            MailServiceStreamStarted { epoch }
        ))
    );
    assert!(state.reserve_mail_cost((epoch, 70), MAX_NATIVE_BUFFER_BYTES));
    let before_bytes = state.pending_bytes();
    let delta = std::mem::size_of::<MailServiceDelivery>()
        - std::mem::size_of::<(MailServiceStreamEpoch, u32)>();
    let unused = std::mem::size_of::<MailQuoteReceipt>()
        + 2 * std::mem::size_of::<MailSendReceipt>()
        + std::mem::size_of::<MailSendAcknowledgement>();
    let bundle_bytes = native_message_bytes(&bundle);
    let cap = before_bytes + bundle_bytes + delta + unused;
    assert!(!state.enqueue_with_limits(bundle.clone(), bundle_bytes, cap - 1));
    assert_eq!(state.pending_bytes(), before_bytes);
    assert_eq!(state.mail_cost_reserve, Some((epoch, 70)));
    assert!(state.enqueue_with_limits(bundle, bundle_bytes, cap));
    assert!(matches!(
        state.pending.front(),
        Some(NativeInboundMessage::MailServiceStreamStarted(_))
    ));
    assert!(is_valid_mail_cost(&state.pending[1]));
    assert!(matches!(
        state.pending[2],
        NativeInboundMessage::HuntingMaterialBundle(_)
    ));
    assert_eq!(state.pending_bytes(), cap - unused);
}

#[test]
fn native_hunting_material_model_equal_host_deliveries_observe_once_each_without_asset_authority() {
    let _serial = native_queue_test_guard();
    let inbound = NativeInbound::new();
    let model = HuntingMaterialModel {
        protocol_v1: true,
        phase: HuntingMaterialPhase::Pending,
        quote_id: Some("ab".repeat(32)),
        ..Default::default()
    };
    let encoded = serde_json::to_string(&model).unwrap();
    assert!(push_native_hunting_material_model(encoded.clone()));
    assert!(push_native_hunting_material_model(encoded));
    assert_eq!(
        inbound.buffer.lock().unwrap().pending.len(),
        2,
        "equal deliveries cannot coalesce"
    );
    assert!(!push_native_hunting_material_model(
        "{\"unknown\":1}".into()
    ));
    let mut world = World::new();
    world.insert_resource(inbound);
    world
        .run_system_once(crate::ingest_pending_hunting_material_model)
        .unwrap();
    assert_eq!(world.resource::<HuntingMaterialModel>(), &model);
    assert_eq!(
        world
            .resource::<NativeInbound>()
            .buffer
            .lock()
            .unwrap()
            .pending
            .len(),
        1
    );
    assert!(
        !world.contains_resource::<InventoryModel>()
            && !world.contains_resource::<NativeHuntingMaterialSource>()
    );
    let first = world
        .get_resource_ref::<HuntingMaterialModel>()
        .unwrap()
        .last_changed();
    world.increment_change_tick();
    world
        .run_system_once(crate::ingest_pending_hunting_material_model)
        .unwrap();
    let second = world
        .get_resource_ref::<HuntingMaterialModel>()
        .unwrap()
        .last_changed();
    assert_ne!(first, second);
    world.increment_change_tick();
    world
        .run_system_once(crate::ingest_pending_hunting_material_model)
        .unwrap();
    assert_eq!(
        world
            .get_resource_ref::<HuntingMaterialModel>()
            .unwrap()
            .last_changed(),
        second,
        "empty frames cannot fabricate observe delivery"
    );
    world.insert_resource(crate::SceneResetRevision(1));
    world
        .run_system_once(crate::ingest_pending_hunting_material_model)
        .unwrap();
    assert_eq!(
        world.resource::<HuntingMaterialModel>(),
        &HuntingMaterialModel::default()
    );

    let mut state = local();
    assert!(state.enqueue(NativeInboundMessage::HuntingMaterialModel("{}".into())));
    assert!(!state.evict_oldest_non_boundary());
    assert!(!state.evict_oldest_non_ack_non_boundary());
    assert!(!state.evict_oldest_coalescible_snapshot());
}

#[test]
fn native_hunting_material_model_prefix_waits_for_real_deliveries_before_complete_bundle() {
    let (gate, bundle) = material();
    let mut state = local();
    let model = serde_json::to_string(&HuntingMaterialModel {
        phase: HuntingMaterialPhase::Pending,
        ..Default::default()
    })
    .unwrap();
    assert!(state.enqueue(NativeInboundMessage::HuntingMaterialModel(model.clone())));
    assert!(state.enqueue(NativeInboundMessage::HuntingMaterialModel(model)));
    assert!(state.enqueue(bundle));
    let mut world = runtime_world(state);
    world
        .run_system_once(crate::ingest_pending_hunting_material_model)
        .unwrap();
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(gate.try_recv_applied().is_none());
    world
        .run_system_once(crate::ingest_pending_hunting_material_model)
        .unwrap();
    crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
    assert!(gate.try_recv_applied().is_some());
    assert_eq!(
        world.resource::<HuntingMaterialModel>().phase,
        HuntingMaterialPhase::Pending,
        "actual opaque proof is still required by the host before publishing Committed"
    );
}

#[test]
fn native_hunting_material_mail_epoch_pressure_cannot_evict_owner_barrier_or_model_delivery() {
    let (gate, bundle) = material();
    let mut state = local();
    assert!(state.enqueue(bundle));
    assert!(state.enqueue(NativeInboundMessage::HuntingMaterialModel("{}".into())));
    let bytes = state.pending_bytes();
    let count = state.message_count();
    let marker = MailServiceStreamStarted {
        epoch: MailServiceStreamEpoch {
            run: 4,
            connection: 5,
        },
    };
    let exact =
        bytes + std::mem::size_of::<MailServiceStreamStarted>() + mail_terminal_reserve_bytes();
    assert!(!state.start_mail_stream(marker, exact - 1));
    assert_eq!(
        (state.pending_bytes(), state.message_count()),
        (bytes, count)
    );
    assert!(state.mail_stream_epoch.is_none());
    assert!(gate.epoch().is_some() && gate.try_recv_applied().is_none());
    assert!(state.start_mail_stream(marker, exact));
    assert_eq!(state.mail_stream_epoch, Some(marker.epoch));
    assert!(state.pending.iter().any(is_owner_bundle));
    assert!(state.pending.iter().any(is_material_model));
}
