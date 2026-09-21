use super::*;

#[test]
fn mail_service_events_are_ordered_and_clear_on_session_reset() {
    let _native_queue_guard = native_ingest::native_queue_test_guard();
    let mut app = App::new();
    app.add_plugins(Mir2NativeSessionBoundaryPlugin)
        .add_systems(Update, ingest_pending_mail_service);

    assert!(native_ingest::push_native_mail_service(
        r#"{"kind":"openParcel"}"#.to_owned()
    ));
    assert!(native_ingest::push_native_mail_service(
        r#"{"kind":"cost","cost":125}"#.to_owned()
    ));
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<mir2_client_bevy::mail_service::MailServiceInbox>()
            .drain(),
        vec![
            mir2_client_bevy::mail_service::MailServiceEvent::OpenParcel,
            mir2_client_bevy::mail_service::MailServiceEvent::Cost { cost: 125 },
        ]
    );

    assert!(native_ingest::push_native_mail_service(
        r#"{"kind":"lockedItem","uniqueId":77,"locked":true}"#.to_owned()
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<mir2_client_bevy::mail_service::MailServiceInbox>()
            .len(),
        1
    );
    assert!(native_ingest::push_native_data_reset());
    app.update();
    assert!(app
        .world()
        .resource::<mir2_client_bevy::mail_service::MailServiceInbox>()
        .is_empty());
}
