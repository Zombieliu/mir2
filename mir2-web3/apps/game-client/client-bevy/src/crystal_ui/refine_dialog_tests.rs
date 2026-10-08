use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};

fn item(uid: u64, container: u8, slot: u32, pending_refine: bool) -> ItemModel {
    ItemModel {
        unique_id: Some(uid),
        name: "Dagger".into(),
        key: "dagger".into(),
        quantity: 1,
        container,
        slot,
        icon: 1,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_type: 1,
                required_amount: 20,
                image: 1,
                ..Default::default()
            },
            user_item: Some(CrystalUserItemModel {
                unique_id: uid,
                refine_added: u8::from(pending_refine),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn context() -> RefineContext {
    RefineContext {
        npc_object_id: 1149,
        map_epoch: 7,
    }
}
fn open(mode: RefineMode) -> RefineDialogUi {
    let mut ui = RefineDialogUi::default();
    ui.begin_service_request(
        context(),
        if mode == RefineMode::Refine {
            "@Refine"
        } else {
            "@RefineCheck"
        },
    );
    ui.observe(&if mode == RefineMode::Refine {
        ServerPacket::NPCRefine {
            rate: 125.0,
            refining: false,
        }
    } else {
        ServerPacket::NPCCheckRefine
    });
    assert!(ui.open);
    ui
}
fn inventory() -> InventoryModel {
    InventoryModel {
        capacity: 86,
        gold: 1_000_000,
        items: vec![
            item(1001, 0, 0, false),
            item(1002, 0, 79, false),
            item(1003, 1, 5, false),
        ],
    }
}

#[test]
fn unsolicited_and_changed_map_npc_menus_do_not_open_refinement() {
    let mut ui = RefineDialogUi::default();
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    ui.observe(&ServerPacket::NPCCheckRefine);
    assert!(!ui.open);
    ui.begin_service_request(context(), "@Refine");
    ui.retain_context(Some(999), 7);
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    assert!(!ui.open);
    ui.begin_service_request(context(), "@Refine");
    ui.retain_context(None, 8);
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    assert!(!ui.open);
    ui.begin_service_request(context(), "@Refine");
    ui.retain_context(None, 7); // A packet-only frame is not an NPC close.
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    assert!(ui.open);
}

#[test]
fn bag2_and_belt_material_transfers_wait_for_matching_ack_and_preserve_uid() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    assert!(ui.select_inventory(&inventory, 85));
    let packet = ui.deposit_selected(&inventory, 15).unwrap();
    assert_eq!(packet, ClientPacket::DepositRefineItem { from: 85, to: 15 });
    assert!(ui.materials[15].is_none());
    assert_eq!(inventory.items[1].unique_id, Some(1002));
    assert!(ui.can_dispatch(&packet, &inventory, Some(1149), 7));
    ui.mark_sent(&packet);
    assert!(!ui.can_dispatch(&packet, &inventory, Some(1149), 7));
    ui.observe(&ServerPacket::DepositRefineItem {
        from: 84,
        to: 15,
        success: true,
    });
    assert!(ui.pending());
    ui.observe(&ServerPacket::DepositRefineItem {
        from: 85,
        to: 15,
        success: true,
    });
    assert_eq!(ui.materials[15].as_ref().unwrap().unique_id, Some(1002));
    assert!(!ui.pending());
    assert!(ui.select_inventory(&inventory, 5));
    let packet = ui.deposit_selected(&inventory, 0).unwrap();
    assert_eq!(packet, ClientPacket::DepositRefineItem { from: 5, to: 0 });
    ui.observe(&ServerPacket::DepositRefineItem {
        from: 5,
        to: 0,
        success: false,
    });
    assert!(ui.materials[0].is_none());
    assert!(!ui.pending());
}

#[test]
fn source_replacement_cell_change_and_old_npc_never_dispatch_stale_refine_commands() {
    let mut inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 85);
    let packet = ui.deposit_selected(&inventory, 0).unwrap();
    inventory.items[1].unique_id = Some(2000);
    assert!(!ui.can_dispatch(&packet, &inventory, Some(1149), 7));
    ui.release_unsent(&packet);
    assert!(!ui.pending());
    ui.select_inventory(&inventory, 85);
    let packet = ui.deposit_selected(&inventory, 0).unwrap();
    assert!(!ui.can_dispatch(&packet, &inventory, Some(1150), 7));
    assert!(!ui.can_dispatch(&packet, &inventory, Some(1149), 8));
    inventory.items[1].slot = 78;
    assert!(!ui.can_dispatch(&packet, &inventory, Some(1149), 7));
    ui.release_unsent(&packet);
    ui.select_inventory(&inventory, 6);
    assert!(ui.select_target(&inventory));
    inventory.items[0].unique_id = Some(3000);
    assert!(ui.confirm(&inventory).is_none());
}

#[test]
fn refine_quote_and_ordinary_check_require_actual_useritem_metadata() {
    let mut inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 6);
    assert!(ui.select_target(&inventory));
    assert_eq!(ui.quote(), Some(25_000));
    assert!(
        ui.confirm(&inventory).is_none(),
        "native source requires material cells"
    );
    ui.observe_custody(&RefineCustodyReadback {
        materials: vec![(0, item(2001, 0, 1, false))],
        ..Default::default()
    });
    inventory.gold = 24_999;
    assert!(ui.confirm(&inventory).is_none());
    assert!(!ui.pending());
    inventory.gold = 25_000;
    let packet = ui.confirm(&inventory).unwrap();
    assert_eq!(packet, ClientPacket::RefineItem { unique_id: 1001 });
    assert_eq!(
        inventory.gold, 25_000,
        "queued command never debits client wallet"
    );
    assert!(
        ui.confirm(&inventory).is_none(),
        "no duplicate start while awaiting server"
    );
    ui.observe(&ServerPacket::NPCCollectRefine { success: false });
    assert_eq!(ui.materials[0].as_ref().unwrap().unique_id, Some(2001));
    assert_eq!(ui.target.as_ref().unwrap().unique_id, 1001);
    assert!(!ui.pending());
    let mut check = open(RefineMode::Check);
    check.select_inventory(&inventory, 6);
    assert!(!check.select_target(&inventory));
    inventory.items[0] = item(1001, 0, 0, true);
    check.select_inventory(&inventory, 6);
    assert!(check.select_target(&inventory));
    let packet = check.confirm(&inventory).unwrap();
    assert_eq!(packet, ClientPacket::CheckRefine { unique_id: 1001 });
    check.observe(&ServerPacket::RefineItem { unique_id: 999 });
    assert!(check.pending());
    check.observe(&ServerPacket::RefineItem { unique_id: 1001 });
    assert!(!check.pending());
    assert!(!check.open);
}

#[test]
fn partial_cancel_full_bag_reject_and_reconnect_never_erase_held_items() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    let readback = RefineCustodyReadback {
        materials: vec![(0, item(2001, 0, 1, false)), (15, item(2002, 0, 2, false))],
        refining: false,
        remaining_ms: 0,
    };
    assert!(ui.observe_custody(&readback));
    ui.defer_cancel();
    let cancel = ui.deferred_cancel().unwrap();
    assert_eq!(cancel, ClientPacket::RefineCancel);
    assert!(ui.can_dispatch(&cancel, &inventory, None, 999));
    ui.mark_sent(&cancel);
    assert!(ui.deferred_cancel().is_none());
    ui.observe(&ServerPacket::RetrieveRefineItem {
        from: 0,
        to: 6,
        success: true,
    });
    assert!(ui.materials[0].is_none());
    assert_eq!(ui.materials[15].as_ref().unwrap().unique_id, Some(2002));
    ui.observe(&ServerPacket::NPCCollectRefine { success: false });
    assert!(!ui.pending());
    assert_eq!(ui.materials[15].as_ref().unwrap().unique_id, Some(2002));
    ui.defer_cancel();
    assert!(ui.deferred_cancel().is_some());
    ui.observe(&ServerPacket::RefineCancel);
    assert!(ui.materials.iter().all(Option::is_none));
    let mut reconnect = RefineDialogUi::default();
    assert!(reconnect.observe_custody(&readback));
    assert_eq!(
        reconnect.materials[15].as_ref().unwrap().unique_id,
        Some(2002)
    );
    let bad = RefineCustodyReadback {
        materials: vec![(0, item(9, 0, 0, false)), (1, item(9, 0, 1, false))],
        ..Default::default()
    };
    assert!(!reconnect.observe_custody(&bad));
    assert_eq!(
        reconnect.materials[15].as_ref().unwrap().unique_id,
        Some(2002)
    );
}

#[test]
fn closing_during_deposit_defers_exactly_one_cancel_until_ack() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 85);
    let deposit = ui.deposit_selected(&inventory, 15).unwrap();
    ui.mark_sent(&deposit);
    ui.defer_cancel();
    assert!(ui.deferred_cancel().is_none());
    ui.observe(&ServerPacket::DepositRefineItem {
        from: 85,
        to: 15,
        success: true,
    });
    assert_eq!(ui.deferred_cancel(), Some(ClientPacket::RefineCancel));
    assert!(ui.deferred_cancel().is_none());
}

#[test]
fn queue_backpressure_keeps_custody_and_can_retry_selected_target() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 85);
    let packet = ui.deposit_selected(&inventory, 0).unwrap();
    ui.release_unsent(&packet);
    assert!(ui.materials[0].is_none());
    assert!(ui.deposit_selected(&inventory, 0).is_some());
    assert_eq!(inventory.items[1].unique_id, Some(1002));
}

#[test]
fn unknown_sent_deposit_keeps_last_uid_and_never_automatically_cancels_or_retries() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    let held = item(2001, 0, 1, false);
    ui.observe_custody(&RefineCustodyReadback {
        materials: vec![(0, held.clone())],
        ..Default::default()
    });
    assert!(ui.select_inventory(&inventory, 85));
    let packet = ui.deposit_selected(&inventory, 15).unwrap();
    ui.mark_sent(&packet);
    ui.defer_cancel();
    assert!(ui.request_in_flight());
    assert!(ui.mark_request_outcome_unknown());
    assert!(!ui.pending());
    assert!(!ui.request_in_flight());
    assert!(ui.requires_authoritative_readback());
    assert!(!ui.open);
    assert_eq!(ui.materials[0], Some(held));
    assert!(ui.materials[15].is_none());
    assert_eq!(inventory.items[1].unique_id, Some(1002));
    assert!(ui.deferred_cancel().is_none());
    assert!(ui.request_cancel().is_none());
    assert!(!ui.can_dispatch(&packet, &inventory, Some(1149), 7));
    ui.observe(&ServerPacket::DepositRefineItem {
        from: 85,
        to: 15,
        success: true,
    });
    ui.observe(&ServerPacket::RefineCancel);
    assert!(
        ui.materials[15].is_none(),
        "a late ACK is not a current command"
    );
    assert_eq!(ui.materials[0].as_ref().unwrap().unique_id, Some(2001));
    assert!(ui.requires_authoritative_readback());
}

#[test]
fn unknown_outcome_needs_new_refine_response_then_valid_authoritative_custody() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 85);
    let packet = ui.deposit_selected(&inventory, 15).unwrap();
    ui.mark_sent(&packet);
    ui.mark_request_outcome_unknown();
    let committed = RefineCustodyReadback {
        materials: vec![(15, item(1002, 0, 79, false))],
        ..Default::default()
    };
    // A delayed full world snapshot alone does not retire the old request.
    assert!(ui.observe_custody(&committed));
    assert!(ui.requires_authoritative_readback());
    ui.begin_service_request(context(), "@RefineCheck");
    ui.observe(&ServerPacket::NPCCheckRefine);
    ui.begin_service_request(context(), "@RefineCollect");
    ui.observe(&ServerPacket::NPCCollectRefine { success: true });
    assert!(!ui.open);
    assert!(ui.requires_authoritative_readback());
    ui.begin_service_request(context(), "@Refine");
    // This is also the bridge's custody-before-service-response same-frame
    // ordering: only a subsequent normal world readback may complete recovery.
    assert!(ui.observe_custody(&committed));
    assert!(ui.requires_authoritative_readback());
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    assert!(!ui.open);
    assert!(!ui.select_material(15));
    let invalid = RefineCustodyReadback {
        materials: vec![(0, item(9, 0, 0, false)), (1, item(9, 0, 1, false))],
        ..Default::default()
    };
    assert!(!ui.observe_custody(&invalid));
    assert!(ui.requires_authoritative_readback());
    assert_eq!(ui.materials[15].as_ref().unwrap().unique_id, Some(1002));
    assert!(ui.observe_custody(&committed));
    assert!(!ui.requires_authoritative_readback());
    assert!(ui.open);
    // The actual inventory no longer contains the deposited UID. Recovery
    // never invents an inventory return; only an explicit user retrieval does.
    let mut authoritative_inventory = inventory.clone();
    authoritative_inventory
        .items
        .retain(|i| i.unique_id != Some(1002));
    assert!(ui.select_material(15));
    let retrieve = ui.retrieve_selected(&authoritative_inventory, 85).unwrap();
    assert_eq!(
        retrieve,
        ClientPacket::RetrieveRefineItem { from: 15, to: 85 }
    );
    assert!(ui.can_dispatch(&retrieve, &authoritative_inventory, Some(1149), 7));
    assert_eq!(ui.materials[15].as_ref().unwrap().unique_id, Some(1002));
}

#[test]
fn unknown_started_oven_uses_real_timer_readback_and_cannot_start_a_second_oven() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.observe_custody(&RefineCustodyReadback {
        materials: vec![(0, item(2001, 0, 1, false))],
        ..Default::default()
    });
    ui.select_inventory(&inventory, 6);
    ui.select_target(&inventory);
    let start = ui.confirm(&inventory).unwrap();
    assert_eq!(start, ClientPacket::RefineItem { unique_id: 1001 });
    ui.mark_sent(&start);
    assert!(ui.mark_request_outcome_unknown());
    assert_eq!(ui.materials[0].as_ref().unwrap().unique_id, Some(2001));
    assert!(
        !ui.refining,
        "no success is inferred from the transport error"
    );
    ui.observe(&ServerPacket::RefineItem { unique_id: 1001 });
    assert!(!ui.refining);
    ui.begin_service_request(context(), "@Refine");
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: true,
    });
    assert!(ui.requires_authoritative_readback());
    assert!(ui.observe_custody(&RefineCustodyReadback {
        materials: Vec::new(),
        refining: true,
        remaining_ms: 1_199_500,
    }));
    assert!(!ui.requires_authoritative_readback());
    assert!(ui.refining);
    assert_eq!(ui.remaining_ms, 1_199_500);
    assert!(ui.materials.iter().all(Option::is_none));
    assert!(!ui.open);
    assert!(ui.confirm(&inventory).is_none());
    assert!(ui.deferred_cancel().is_none());
    assert!(!ui.can_dispatch(&start, &inventory, Some(1149), 7));
}

#[test]
fn recovery_requires_original_npc_and_new_context_response_before_readback() {
    let inventory = inventory();
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 5);
    let deposit = ui.deposit_selected(&inventory, 0).unwrap();
    ui.mark_sent(&deposit);
    ui.mark_request_outcome_unknown();
    ui.retain_context(None, 8);
    ui.begin_service_request(
        RefineContext {
            npc_object_id: 999,
            map_epoch: 8,
        },
        "@Refine",
    );
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    ui.observe_custody(&RefineCustodyReadback::default());
    assert!(ui.requires_authoritative_readback());
    let returned_context = RefineContext {
        map_epoch: 9,
        ..context()
    };
    ui.begin_service_request(returned_context, "@Refine");
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    // Closing before the readback retires that response's presentation lease.
    ui.defer_cancel();
    ui.observe_custody(&RefineCustodyReadback::default());
    assert!(ui.requires_authoritative_readback());
    assert!(ui.deferred_cancel().is_none());
    ui.begin_service_request(returned_context, "@Refine");
    ui.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    ui.observe_custody(&RefineCustodyReadback::default());
    assert!(!ui.requires_authoritative_readback());
    assert!(ui.open);
}

#[test]
fn timeout_is_exact_and_does_not_expire_unsent_backpressure_or_acknowledged_requests() {
    let inventory = inventory();
    let base = Instant::now();
    let timeout = Duration::from_secs(15);
    let mut ui = open(RefineMode::Refine);
    ui.select_inventory(&inventory, 85);
    let deposit = ui.deposit_selected(&inventory, 15).unwrap();
    ui.request_started_at = Some(base);
    assert!(!ui.request_in_flight());
    assert!(!ui.mark_request_outcome_unknown());
    assert!(!ui.expire_request_outcome(base + timeout * 2, timeout));
    ui.mark_sent(&deposit);
    ui.request_started_at = Some(base);
    ui.mark_sent(&deposit);
    assert_eq!(
        ui.request_started_at,
        Some(base),
        "duplicate marking cannot extend the deadline"
    );
    assert!(!ui.expire_request_outcome(base - Duration::from_nanos(1), timeout));
    assert!(!ui.expire_request_outcome(base + timeout - Duration::from_nanos(1), timeout));
    assert!(ui.expire_request_outcome(base + timeout, timeout));
    assert!(ui.requires_authoritative_readback());
    assert!(!ui.expire_request_outcome(base + timeout * 2, timeout));

    let mut acknowledged = open(RefineMode::Refine);
    acknowledged.select_inventory(&inventory, 85);
    let deposit = acknowledged.deposit_selected(&inventory, 15).unwrap();
    acknowledged.mark_sent(&deposit);
    acknowledged.request_started_at = Some(base);
    acknowledged.observe(&ServerPacket::DepositRefineItem {
        from: 85,
        to: 15,
        success: true,
    });
    assert!(!acknowledged.expire_request_outcome(base + timeout * 2, timeout));
    assert!(!acknowledged.requires_authoritative_readback());
    assert_eq!(
        acknowledged.materials[15].as_ref().unwrap().unique_id,
        Some(1002)
    );

    let mut service = RefineDialogUi::default();
    service.begin_service_request(context(), "@Refine");
    service.request_started_at = Some(base);
    assert!(service.request_in_flight());
    assert!(service.expire_request_outcome(base + timeout, timeout));
    assert!(service.requires_authoritative_readback());
}

#[test]
fn reconnect_generation_reset_ignores_all_retired_refine_acknowledgements() {
    let inventory = inventory();
    let mut previous = open(RefineMode::Refine);
    previous.select_inventory(&inventory, 85);
    let deposit = previous.deposit_selected(&inventory, 15).unwrap();
    previous.mark_sent(&deposit);
    previous.mark_request_outcome_unknown();
    // The bridge replaces the model on the existing transport generation
    // boundary before draining current-generation snapshots.
    let mut current = RefineDialogUi::default();
    current.observe(&ServerPacket::DepositRefineItem {
        from: 85,
        to: 15,
        success: true,
    });
    current.observe(&ServerPacket::RefineCancel);
    current.observe(&ServerPacket::RefineItem { unique_id: 1001 });
    current.observe(&ServerPacket::NPCRefine {
        rate: 125.0,
        refining: false,
    });
    assert!(!current.open);
    assert!(!current.pending());
    assert!(current.materials.iter().all(Option::is_none));
    assert!(current.observe_custody(&RefineCustodyReadback {
        materials: vec![(15, item(1002, 0, 79, false))],
        ..Default::default()
    }));
    assert_eq!(
        current.materials[15].as_ref().unwrap().unique_id,
        Some(1002)
    );
    assert!(
        !current.open,
        "reconnect readback does not create an NPC lease"
    );
}
