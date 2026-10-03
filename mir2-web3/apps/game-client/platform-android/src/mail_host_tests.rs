//! Headless tests of the real host receive/native-model producer.
//! These are neither JNI/GPU evidence nor authenticated online mail acceptance.
use super::*;
use mir2_client_bevy::{
    mail::{MailMessage, MailModel, MAX_MAIL_ATTACHMENTS, MAX_MAIL_MESSAGES},
    mail_service::{MailServiceEvent, MailServiceInbox},
};

fn app() -> App {
    INBOX.lock().unwrap().clear();
    OUTBOX.lock().unwrap().clear();
    let mut app = App::new();
    app.add_plugins((
        mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin,
        mir2_bevy_runtime::Mir2NativeMailIngressPlugin,
    ));
    mir2_bevy_runtime::native_ingest::install_native_ingestion(&mut app);
    #[cfg(feature = "ui-preview")]
    app.init_resource::<crate::ui_preview::PreviewRequest>();
    app.insert_resource(NativeShellModel {
        screen: Screen::StartingGame,
        ..default()
    })
    .insert_resource(HostState {
        phase: "STARTING".into(),
        ..default()
    })
    .init_resource::<NativeUiIntentQueue>()
    .add_systems(PreUpdate, receive);
    app
}
fn host_packet(packet: &str, payload: Value) -> Value {
    json!({"type":"gatewayGameplayPacket","envelope":
        json!({"type":"packet","packet":packet,"payload":payload}).to_string()})
}
fn row(id: u64) -> Value {
    json!({"mailId":id.to_string(),"senderName":"NPC","message":"Subject\nBody",
        "items":[],"canReply":true,"dateSentBinaryDatetime":"638970336000000000"})
}
fn host_world(owner: u32, name: &str, map: &str, mails: Option<Vec<Value>>) -> Value {
    let mut raw = json!({"playerObjectId":owner,"mapFileName":map,
        "entities":[{"kind":"selfPlayer","objectId":owner,"name":name,"x":10,"y":20}]});
    if let Some(mails) = mails {
        raw["stage5Systems"] = json!({"mail":mails});
    }
    json!({"phase":"IN_GAME","world":{"playerName":name,"mapFileName":map,"x":10,"y":20},
        "worldSnapshot":raw.to_string()})
}
fn assert_terminal(app: &App) {
    let host = app.world().resource::<HostState>();
    assert_eq!(host.phase, "DISCONNECTED");
    assert_eq!(host.mail.pending_count(), 0);
    assert!(host.world.is_none() && host.pending_render_request.is_none());
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::ConnectionLost
    );
    assert!(app.world().resource::<MailModel>().mails.is_empty());
    assert!(app.world().resource::<MailServiceInbox>().is_empty());
    let mut out = OUTBOX.lock().unwrap();
    let command: Value = serde_json::from_str(&out.pop_front().unwrap()).unwrap();
    assert_eq!(command["type"], "disconnect");
    assert!(out.is_empty());
}

#[test]
fn mail_host_phase_and_failed_render_retire_pending_without_granting_world() {
    let raw = json!({"type":"packet","packet":"MailCost","payload":{"cost":125}}).to_string();
    let mut host = HostState::default();
    for phase in ["DISCONNECTED", "READY", "CHARACTERS"] {
        host.phase = phase.into();
        assert!(!host.accept_mail_packet(Screen::StartingGame, &raw).unwrap());
    }
    host.phase = "STARTING".into();
    for screen in [
        Screen::Login,
        Screen::CharacterSelect,
        Screen::ConnectionLost,
    ] {
        assert!(!host.accept_mail_packet(screen, &raw).unwrap());
    }
    assert!(host.accept_mail_packet(Screen::StartingGame, &raw).unwrap());
    assert_eq!(host.mail.pending_count(), 1);
    assert!(host.world.is_none());
    host.pending_render_request = Some(7);
    let mut shell = NativeShellModel {
        screen: Screen::StartingGame,
        ..default()
    };
    assert!(fail_current_render_load(
        &mut host,
        &mut shell,
        7,
        "OFFLINE mail fixture failure"
    ));
    assert_eq!(host.mail.pending_count(), 0);
    assert!(!host
        .mail
        .flush(|_| panic!("retired model"), |_| panic!("retired service")));
}

#[test]
fn mail_host_packet_first_models_wait_for_owner_and_keep_render_barrier() {
    let mut app = app();
    INBOX.lock().unwrap().extend([
        host_packet("ReceiveMail", json!({"mail":[row(2)]})),
        host_packet("MailCost", json!({"cost":125})),
        host_packet("MailSendRequest", Value::Null),
    ]);
    app.update();
    assert_eq!(app.world().resource::<HostState>().mail.pending_count(), 3);
    assert!(app.world().resource::<MailModel>().mails.is_empty());
    assert!(app.world().resource::<MailServiceInbox>().is_empty());
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(42, "Fixture", "0", Some(vec![row(1)])));
    app.update();
    app.update();
    let host = app.world().resource::<HostState>();
    assert_eq!(host.mail.pending_count(), 0);
    assert_eq!(host.phase, "IN_GAME");
    assert!(host.pending_render_request.is_some());
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::StartingGame
    );
    // The native mailbox queue coalesces snapshots; latest packet follows the base.
    let mails = &app.world().resource::<MailModel>().mails;
    assert_eq!(mails.len(), 1);
    assert_eq!(mails[0].id, 2);
    assert!(mails[0].operation.is_none());
    assert_eq!(
        app.world_mut().resource_mut::<MailServiceInbox>().drain(),
        [
            MailServiceEvent::Cost { cost: 125 },
            MailServiceEvent::OpenParcel
        ]
    );
    assert!(OUTBOX.lock().unwrap().is_empty());
}

#[test]
fn mail_host_full_256_by_five_survives_large_escaped_envelope_and_native_ingestion() {
    let mut app = app();
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(42, "Fixture", "0", None));
    app.update();
    let rows: Vec<_> = (1..=MAX_MAIL_MESSAGES)
        .map(|id| {
            let mut r = row(id as u64);
            r["message"] = json!("邮件正文".repeat(20));
            r["items"] = json!((1..=MAX_MAIL_ATTACHMENTS)
                .map(|n| json!({"uniqueId":(id*10+n).to_string(),"name":"RedPotion","count":n}))
                .collect::<Vec<_>>());
            r
        })
        .collect();
    let text = host_packet("ReceiveMail", json!({"mail":rows})).to_string();
    assert!(text.len() > 65536);
    enqueue_host_event(&mut INBOX.lock().unwrap(), &text);
    assert_eq!(INBOX.lock().unwrap()[0]["type"], "gatewayGameplayPacket");
    app.update();
    app.update();
    let model = app.world().resource::<MailModel>();
    assert_eq!(model.mails.len(), MAX_MAIL_MESSAGES);
    assert_eq!(model.mails.last().unwrap().id, MAX_MAIL_MESSAGES as u64);
    assert!(model
        .mails
        .iter()
        .all(|mail| mail.items.len() == MAX_MAIL_ATTACHMENTS));
    assert_eq!(
        model.mails.last().unwrap().items.last().unwrap().unique_id,
        Some(2565)
    );
    assert!(model.mails.iter().all(|mail| mail.operation.is_none()));
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::StartingGame
    );
    assert!(OUTBOX.lock().unwrap().is_empty());
}

#[test]
fn mail_host_invalid_batch_clears_models_services_and_skips_later_world() {
    let mut app = app();
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(42, "Fixture", "0", None));
    app.update();
    app.world_mut()
        .resource_mut::<MailModel>()
        .mails
        .push(MailMessage {
            id: 99,
            ..default()
        });
    app.world_mut()
        .resource_mut::<MailServiceInbox>()
        .push(MailServiceEvent::OpenParcel);
    INBOX.lock().unwrap().extend([
        host_packet("ReceiveMail", json!({"mail":[row(1),row(1)]})),
        host_packet("MailCost", json!({"cost":321})),
        host_world(42, "Fixture", "0", Some(vec![row(2)])),
    ]);
    app.update();
    app.update();
    assert_terminal(&app);
}

#[test]
fn mail_host_owner_switch_revokes_previous_character_without_inventing_success() {
    let mut app = app();
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(42, "Fixture", "0", Some(vec![row(1)])));
    app.update();
    app.update();
    assert_eq!(app.world().resource::<MailModel>().mails[0].id, 1);
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(43, "Other", "0", Some(vec![row(2)])));
    app.update();
    app.update();
    assert_terminal(&app);
}

#[test]
fn mail_host_rejected_start_discards_staged_data_before_another_character() {
    let mut app = app();
    INBOX
        .lock()
        .unwrap()
        .push_back(host_packet("ReceiveMail", json!({"mail":[row(1)]})));
    app.update();
    assert_eq!(app.world().resource::<HostState>().mail.pending_count(), 1);
    INBOX
        .lock()
        .unwrap()
        .push_back(json!({"phase":"CHARACTERS","message":"OFFLINE rejected Start"}));
    app.update();
    assert_eq!(app.world().resource::<HostState>().mail.pending_count(), 0);
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::CharacterSelect
    );
    app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame;
    app.world_mut().resource_mut::<HostState>().phase = "STARTING".into();
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(43, "Other", "1", None));
    app.update();
    app.update();
    assert!(app.world().resource::<MailModel>().mails.is_empty());
    assert!(app.world().resource::<MailServiceInbox>().is_empty());
    assert!(OUTBOX.lock().unwrap().is_empty());
}

#[test]
fn mail_host_same_owner_map_reset_keeps_personal_data_but_rejects_old_scene_tag() {
    let mut app = app();
    INBOX
        .lock()
        .unwrap()
        .push_back(host_world(42, "Fixture", "0", Some(vec![row(1)])));
    app.update();
    app.update();
    assert_eq!(app.world().resource::<MailModel>().mails[0].id, 1);
    INBOX
        .lock()
        .unwrap()
        .push_back(json!({"phase":"STARTING","message":"OFFLINE map transition"}));
    app.update();
    INBOX.lock().unwrap().extend([
        host_packet("MailCost", json!({"cost":321})),
        host_packet("MailCost", json!({"cost":999,"mapFileName":"0"})),
        host_world(42, "Fixture", "1", None),
    ]);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<MailModel>().mails[0].id, 1);
    assert_eq!(
        app.world_mut().resource_mut::<MailServiceInbox>().drain(),
        [MailServiceEvent::Cost { cost: 321 }]
    );
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::StartingGame
    );
    assert!(OUTBOX.lock().unwrap().is_empty());
}

#[test]
fn mail_host_pending_overflow_is_terminal_across_bounded_frames() {
    let mut app = app();
    let event = host_packet("MailCost", json!({"cost":1}));
    for count in [31, 31, 2] {
        INBOX
            .lock()
            .unwrap()
            .extend(std::iter::repeat_n(event.clone(), count));
        app.update();
    }
    assert_eq!(app.world().resource::<HostState>().mail.pending_count(), 64);
    INBOX
        .lock()
        .unwrap()
        .extend([event, host_world(42, "Fixture", "0", None)]);
    app.update();
    app.update();
    assert_terminal(&app);
}

#[test]
fn mail_host_envelope_exception_keeps_count_byte_and_other_domain_bounds() {
    let mut queue = VecDeque::new();
    let large_mail = host_packet(
        "ReceiveMail",
        json!({"mail":[],"unused":"x".repeat(500_000)}),
    )
    .to_string();
    enqueue_host_event(&mut queue, &large_mail);
    assert_eq!(queue[0]["type"], "gatewayGameplayPacket");
    // The serialized estimate retains the original aggregate 8 MiB ceiling.
    for _ in 0..14 {
        enqueue_host_event(&mut queue, &large_mail);
    }
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0]["phase"], "DISCONNECTED");
    for packet in ["MailCost", "GameShopInfo", "ReceiveMail"] {
        let bytes = if packet == "ReceiveMail" {
            512 * 1024 + 1
        } else {
            65537
        };
        let text = host_packet(packet, json!({"mail":[],"unused":"x".repeat(bytes)})).to_string();
        queue.clear();
        enqueue_host_event(&mut queue, &text);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0]["phase"], "DISCONNECTED", "{packet}");
    }
    queue.clear();
    for _ in 0..32 {
        enqueue_host_event(
            &mut queue,
            &host_packet("MailCost", json!({"cost":1})).to_string(),
        );
    }
    assert_eq!(queue.len(), 32);
    enqueue_host_event(
        &mut queue,
        &host_packet("MailCost", json!({"cost":1})).to_string(),
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0]["phase"], "DISCONNECTED");
}
