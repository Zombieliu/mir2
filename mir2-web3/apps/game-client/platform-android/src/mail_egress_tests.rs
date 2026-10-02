//! Real shared-queue/Android-host seam; not JNI, GPU, or online acceptance.
use super::*;
use crate::{
    android_input::{AndroidLifecycle, AndroidNetwork, AndroidShellState},
    drain_android_gateway_for_host,
    gateway_bridge::{AndroidGatewayHostWriteResult, AndroidGatewayOutboundQueue},
    report_android_gateway_write_result, AndroidShellPlugin,
};
use mir2_client_bevy::{
    crystal_ui::overlays::{NativePlayerUiIntent, NativePlayerUiIntentQueue},
    inventory::{InventoryModel, ItemModel},
    pending_operations::PendingOperations,
    quest_ui::QuestUiIntentQueue,
};

const ITEM: u64 = 9_007_199_254_740_993;

fn app() -> App {
    app_with_boundary(true)
}

fn app_with_boundary(include_runtime_boundary: bool) -> App {
    INBOX.lock().unwrap().clear();
    OUTBOX.lock().unwrap().clear();
    let mut app = App::new();
    app.add_plugins(AndroidShellPlugin);
    if include_runtime_boundary {
        app.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
    }
    mir2_bevy_runtime::native_ingest::install_native_ingestion(&mut app);
    let mut host = HostState {
        phase: "IN_GAME".into(),
        ..default()
    };
    host.player
        .snapshot(
            &json!({"playerObjectId":42,"mapFileName":"0",
        "entities":[{"kind":"selfPlayer","objectId":42,"name":"Fixture","x":10,"y":20}]})
            .to_string(),
        )
        .unwrap();
    host.world = Some(HostWorldPosition {
        player_name: "Fixture".into(),
        map_file_name: "0".into(),
        x: 10,
        y: 20,
    });
    app.insert_resource(host)
        .insert_resource(NativeShellModel {
            screen: Screen::InGame,
            ..default()
        })
        .insert_resource(AndroidShellState {
            lifecycle: AndroidLifecycle::Foreground,
            network: AndroidNetwork::Available,
            ..default()
        })
        .insert_resource(InventoryModel {
            capacity: 86,
            items: vec![ItemModel {
                unique_id: Some(ITEM),
                slot: 47,
                container: 0,
                ..default()
            }],
            ..default()
        })
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<QuestUiIntentQueue>()
        .add_systems(
            PostUpdate,
            (forward_quest_ui_intents, forward_native_mail_ui_intents).chain(),
        );
    if include_runtime_boundary {
        // The real shared renderer also registers this exact UI reset consumer.
        // Runtime model reset alone intentionally cannot clear overlay queues.
        app.init_resource::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>()
            .init_resource::<mir2_client_bevy::crystal_ui::overlays::MailComposeUi>()
            .init_resource::<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<mir2_client_bevy::pending_operations::OverlayResetTracker>()
            .add_systems(
                Update,
                mir2_client_bevy::pending_operations::apply_overlay_session_reset
                    .in_set(mir2_client_bevy::pending_operations::PendingLifecycleSet::UiReset),
            );
    }
    app.world_mut().spawn(Window {
        focused: true,
        ..default()
    });
    app
}

fn push(app: &mut App, intent: NativePlayerUiIntent) {
    app.world_mut()
        .resource_scope(|world, mut pending: Mut<PendingOperations>| {
            assert!(world
                .resource_mut::<NativePlayerUiIntentQueue>()
                .push_pending_intent(&mut pending, intent));
        });
}

#[test]
fn mail_egress_actual_producer_consumes_mail_without_stealing_other_ui_intents() {
    let mut app = app();
    push(&mut app, NativePlayerUiIntent::RefreshFriends);
    push(&mut app, NativePlayerUiIntent::ReadMail { mail_id: ITEM });
    app.update();
    let leases = drain_android_gateway_for_host(&mut app, 16);
    assert_eq!(
        leases.len(),
        1,
        "Shared mail intent never reached the real Android host"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&leases[0].outbound().json).unwrap(),
        json!({"type":"readMail","mailId":ITEM})
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        [NativePlayerUiIntent::RefreshFriends]
    );
}

#[test]
fn mail_egress_cost_preserves_stamp_preference_and_second_bag_uid() {
    let mut app = app();
    push(
        &mut app,
        NativePlayerUiIntent::MailCost {
            gold: u32::MAX,
            attachment_unique_ids: vec![ITEM],
            stamped: true,
        },
    );
    app.update();
    let leases = drain_android_gateway_for_host(&mut app, 16);
    assert_eq!(
        leases.len(),
        1,
        "Parcel quote has no Android shared-UI producer"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&leases[0].outbound().json).unwrap(),
        json!({"type":"mailCost","gold":u32::MAX,"itemsIdx":[ITEM,0,0,0,0],"stamped":true})
    );
}

#[test]
fn mail_egress_host_write_is_not_a_claim_or_send_receipt() {
    for intent in [
        NativePlayerUiIntent::ClaimMail { mail_id: 7 },
        NativePlayerUiIntent::SendMail {
            recipient: "Friend".into(),
            message: "邮件\nhello 👋".into(),
            gold: 7,
            attachment_unique_ids: vec![ITEM],
            stamped: true,
        },
    ] {
        let mut app = app();
        let key = intent.pending_key().unwrap();
        push(&mut app, intent);
        app.update();
        let mut leases = drain_android_gateway_for_host(&mut app, 16);
        assert_eq!(
            leases.len(),
            1,
            "Reserved mail operation cannot reach authenticated host"
        );
        assert!(app.world().resource::<PendingOperations>().contains(&key));
        report_android_gateway_write_result(
            &mut app,
            leases.remove(0),
            AndroidGatewayHostWriteResult::Sent,
        );
        assert!(
            app.world().resource::<PendingOperations>().contains(&key),
            "Socket write is not settlement"
        );
        assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
    }
}

#[test]
fn mail_egress_full_host_queue_terminates_instead_of_stranding_quote_or_replaying() {
    let mut app = app();
    let mut full = AndroidGatewayOutboundQueue::with_capacity(1);
    full.enqueue(mir2_ui_core::effect::GatewayCommand::Logout)
        .unwrap();
    app.insert_resource(full);
    push(&mut app, NativePlayerUiIntent::ClaimMail { mail_id: 7 });
    push(
        &mut app,
        NativePlayerUiIntent::MailCost {
            gold: 7,
            attachment_unique_ids: vec![],
            stamped: false,
        },
    );
    app.update();
    assert_eq!(app.world().resource::<HostState>().phase, "DISCONNECTED");
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::ConnectionLost
    );
    assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
    app.update(); // Existing DataReset consumer retires reserved operations, never grants success.
    assert!(app.world().resource::<PendingOperations>().is_empty());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    assert_eq!(
        serde_json::from_str::<Value>(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap(),
        json!({"type":"disconnect"})
    );
}

#[test]
fn mail_egress_seven_intents_cross_the_actual_host_with_exact_fifo_fields() {
    let cases = [
        (
            NativePlayerUiIntent::ReadMail { mail_id: ITEM },
            json!({"type":"readMail","mailId":ITEM}),
        ),
        (
            NativePlayerUiIntent::LockMail {
                mail_id: 4,
                lock: true,
            },
            json!({"type":"lockMail","mailId":4,"lock":true}),
        ),
        (
            NativePlayerUiIntent::ClaimMail { mail_id: 5 },
            json!({"type":"collectParcel","mailId":5}),
        ),
        (
            NativePlayerUiIntent::DeleteMail { mail_id: 6 },
            json!({"type":"deleteMail","mailId":6}),
        ),
        (
            NativePlayerUiIntent::MailCost {
                gold: 7,
                attachment_unique_ids: vec![ITEM],
                stamped: true,
            },
            json!({"type":"mailCost","gold":7,"itemsIdx":[ITEM,0,0,0,0],"stamped":true}),
        ),
        (
            NativePlayerUiIntent::MailLockItem {
                unique_id: ITEM,
                locked: false,
            },
            json!({"type":"mailLockedItem","uniqueId":ITEM,"locked":false}),
        ),
        (
            NativePlayerUiIntent::SendMail {
                recipient: "Friend".into(),
                message: "邮件\nhello 👋".into(),
                gold: 8,
                attachment_unique_ids: vec![ITEM],
                stamped: true,
            },
            json!({"type":"sendMail","name":"Friend","message":"邮件\nhello 👋","gold":8,
                "itemsIdx":[ITEM,0,0,0,0],"stamped":true}),
        ),
    ];
    // Separate sessions are intentional: uncorrelated claim/send must not run concurrently.
    for (intent, expected) in cases {
        let mut app = app();
        push(&mut app, intent);
        app.update();
        let leases = drain_android_gateway_for_host(&mut app, 1);
        assert_eq!(leases.len(), 1);
        assert_eq!(
            serde_json::from_str::<Value>(&leases[0].outbound().json).unwrap(),
            expected
        );
        assert!(drain_android_gateway_for_host(&mut app, 1).is_empty());
    }
    let mut app = app();
    for id in 1..=20 {
        push(&mut app, NativePlayerUiIntent::ReadMail { mail_id: id });
    }
    app.update();
    let first = drain_android_gateway_for_host(&mut app, 24);
    assert_eq!(first.len(), 16);
    app.update();
    let second = drain_android_gateway_for_host(&mut app, 24);
    assert_eq!(second.len(), 4);
    let ids: Vec<_> = first
        .iter()
        .chain(second.iter())
        .map(|lease| {
            serde_json::from_str::<Value>(&lease.outbound().json).unwrap()["mailId"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(ids, (1..=20).collect::<Vec<_>>());
}

#[test]
fn mail_egress_waits_for_owner_render_foreground_network_and_focus_barriers() {
    for gate in 0..8 {
        let mut app = app();
        match gate {
            0 => app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame,
            1 => app.world_mut().resource_mut::<HostState>().phase = "STARTING".into(),
            2 => {
                app.world_mut()
                    .resource_mut::<HostState>()
                    .pending_render_request = Some(7)
            }
            3 => {
                app.world_mut()
                    .resource_mut::<HostState>()
                    .pending_world_request = Some(7)
            }
            4 => {
                app.world_mut()
                    .resource_mut::<HostState>()
                    .render_load_active = true
            }
            5 => {
                app.world_mut()
                    .resource_mut::<AndroidShellState>()
                    .lifecycle = AndroidLifecycle::Background
            }
            6 => {
                app.world_mut().resource_mut::<AndroidShellState>().network =
                    AndroidNetwork::Unavailable
            }
            _ => {
                let world = app.world_mut();
                let mut query = world.query::<&mut Window>();
                query.single_mut(world).unwrap().focused = false;
            }
        }
        push(&mut app, NativePlayerUiIntent::ReadMail { mail_id: 7 });
        app.update();
        assert!(
            drain_android_gateway_for_host(&mut app, 16).is_empty(),
            "gate {gate}"
        );
        app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::InGame;
        {
            let mut host = app.world_mut().resource_mut::<HostState>();
            host.phase = "IN_GAME".into();
            host.pending_render_request = None;
            host.pending_world_request = None;
            host.render_load_active = false;
        }
        {
            let mut state = app.world_mut().resource_mut::<AndroidShellState>();
            state.lifecycle = AndroidLifecycle::Foreground;
            state.network = AndroidNetwork::Available;
        }
        {
            let world = app.world_mut();
            let mut query = world.query::<&mut Window>();
            query.single_mut(world).unwrap().focused = true;
        }
        app.update();
        assert_eq!(
            drain_android_gateway_for_host(&mut app, 16).len(),
            1,
            "gate {gate}"
        );
    }
    let mut app = app();
    app.world_mut().resource_mut::<HostState>().player.reset();
    push(&mut app, NativePlayerUiIntent::ReadMail { mail_id: 7 });
    app.update();
    assert!(
        drain_android_gateway_for_host(&mut app, 16).is_empty(),
        "IN_GAME string is not an owner grant"
    );
}

#[test]
fn mail_egress_terminal_boundary_retires_unsent_mail_and_never_replays_claims() {
    for screen in [
        Screen::Login,
        Screen::CharacterSelect,
        Screen::ConnectionLost,
    ] {
        let mut app = app();
        let intent = NativePlayerUiIntent::ClaimMail { mail_id: 7 };
        let key = intent.pending_key().unwrap();
        push(&mut app, intent);
        app.world_mut().resource_mut::<NativeShellModel>().screen = screen;
        app.update();
        assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
        assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::InGame;
        app.update();
        assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
    }
    let mut app = app();
    push(&mut app, NativePlayerUiIntent::ClaimMail { mail_id: 7 });
    app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame;
    mir2_bevy_runtime::native_ingest::push_native_scene_reset();
    app.update();
    assert!(app
        .world()
        .resource::<PendingOperations>()
        .has_pending_mail_operation());
    assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
    mir2_bevy_runtime::native_ingest::push_native_data_reset();
    app.update();
    assert!(app.world().resource::<PendingOperations>().is_empty());
    app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::InGame;
    app.update();
    assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
}

#[test]
fn mail_egress_invalid_attachments_retire_the_batch_without_local_custody_changes() {
    for invalid in 0..5 {
        // The runtime's model-reset system requires InventoryModel. Test the
        // producer's missing-resource fail-closed seam without that plugin;
        // all other cases run both actual runtime and shared UI consumers.
        let mut app = app_with_boundary(invalid != 0);
        let mut ids = vec![ITEM];
        match invalid {
            0 => {
                app.world_mut().remove_resource::<InventoryModel>();
            }
            1 => app.world_mut().resource_mut::<InventoryModel>().items[0].container = 1,
            2 => app.world_mut().resource_mut::<InventoryModel>().capacity = 46,
            3 => ids.push(ITEM),
            _ => ids = vec![0],
        }
        let before = app
            .world()
            .get_resource::<InventoryModel>()
            .map(|inventory| serde_json::to_value(inventory).unwrap());
        push(
            &mut app,
            NativePlayerUiIntent::MailCost {
                gold: 7,
                attachment_unique_ids: ids,
                stamped: true,
            },
        );
        push(&mut app, NativePlayerUiIntent::ReadMail { mail_id: 8 });
        app.update();
        assert_eq!(app.world().resource::<HostState>().phase, "DISCONNECTED");
        assert!(drain_android_gateway_for_host(&mut app, 16).is_empty());
        assert_eq!(
            app.world()
                .get_resource::<InventoryModel>()
                .map(|inventory| serde_json::to_value(inventory).unwrap()),
            before
        );
        app.update();
        assert!(app.world().resource::<PendingOperations>().is_empty());
        assert_eq!(OUTBOX.lock().unwrap().len(), 1);
    }
}

#[test]
fn mail_egress_queue_byte_cap_and_full_rejection_keep_old_fifo_and_sequence() {
    use crate::gateway_bridge::{
        AndroidGatewayEnqueueError, ANDROID_NATIVE_MAIL_COMMAND_MAX_BYTES,
    };
    use mir2_client_bevy::native_mail_egress::NativeMailCommand;
    let mut queue = AndroidGatewayOutboundQueue::with_capacity(1);
    let command = NativeMailCommand::ReadMail { mail_id: u64::MAX };
    queue.enqueue_native_mail(&command).unwrap();
    assert!(matches!(
        queue.enqueue_native_mail(&command),
        Err(AndroidGatewayEnqueueError::Full { .. })
    ));
    let state = AndroidShellState {
        lifecycle: AndroidLifecycle::Foreground,
        network: AndroidNetwork::Available,
        ..default()
    };
    let first = queue.drain_ready(&state, 2);
    assert_eq!(first.len(), 1);
    assert_eq!(
        serde_json::from_str::<Value>(&first[0].json).unwrap()["mailId"].as_u64(),
        Some(u64::MAX)
    );
    let huge = NativeMailCommand::SendMail {
        name: "Friend".into(),
        message: "x".repeat(ANDROID_NATIVE_MAIL_COMMAND_MAX_BYTES + 1),
        gold: 0,
        items_idx: [0; 5],
        stamped: false,
    };
    assert_eq!(
        queue.enqueue_native_mail(&huge),
        Err(AndroidGatewayEnqueueError::OversizedNativeMail {
            max_bytes: ANDROID_NATIVE_MAIL_COMMAND_MAX_BYTES
        })
    );
    assert!(queue.is_empty());
    queue.enqueue_native_mail(&command).unwrap();
    assert_eq!(
        queue.drain_ready(&state, 1)[0].sequence,
        first[0].sequence + 1
    );
}

#[test]
fn mail_egress_real_native_copy_callback_and_generation_never_settle_or_replay() {
    crate::mir2_android_gateway_host_start();
    let mut app = app();
    app.world_mut()
        .resource_mut::<crate::AndroidGatewayTransportEnabled>()
        .0 = true;
    let intent = NativePlayerUiIntent::ClaimMail { mail_id: 7 };
    let key = intent.pending_key().unwrap();
    push(&mut app, intent);
    app.update();
    app.update(); // Existing Update host publisher follows PostUpdate UI producer.
    let required =
        unsafe { crate::mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) };
    assert!(required > 0 && required <= 64 * 1024);
    let mut bytes = vec![0; required as usize];
    assert_eq!(
        unsafe { crate::mir2_android_gateway_copy_next_outbound(bytes.as_mut_ptr(), bytes.len()) },
        required
    );
    let envelope: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        envelope["command"],
        json!({"type":"collectParcel","mailId":7})
    );
    let sequence = envelope["sequence"].as_u64().unwrap();
    assert!(crate::mir2_android_gateway_report_write_result(
        sequence, true
    ));
    assert!(!crate::mir2_android_gateway_report_write_result(
        sequence, true
    ));
    app.update();
    assert!(app.world().resource::<PendingOperations>().contains(&key));
    crate::mir2_android_gateway_connection_lost();
    crate::mir2_android_gateway_host_start();
    app.update();
    assert_eq!(
        unsafe { crate::mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) },
        0
    );
    assert!(!crate::mir2_android_gateway_report_write_result(
        sequence, false
    ));
    mir2_bevy_runtime::native_ingest::push_native_data_reset();
    app.update();
    assert!(!app.world().resource::<PendingOperations>().contains(&key));
    crate::mir2_android_gateway_host_stop();
}
