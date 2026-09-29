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

#[test]
fn full_ui_fifo_still_delivers_the_single_flight_cost_and_reset_clears_it() {
    let _native_queue_guard = native_ingest::native_queue_test_guard();
    let mut app = App::new();
    app.add_plugins(Mir2NativeSessionBoundaryPlugin)
        .add_systems(Update, ingest_pending_mail_service);

    {
        let mut inbox = app
            .world_mut()
            .resource_mut::<mir2_client_bevy::mail_service::MailServiceInbox>();
        for unique_id in 0..mir2_client_bevy::mail_service::MAIL_SERVICE_INBOX_CAPACITY {
            assert!(inbox.push(
                mir2_client_bevy::mail_service::MailServiceEvent::LockedItem {
                    unique_id: unique_id as u64 + 1,
                    locked: true,
                },
            ));
        }
    }
    assert!(native_ingest::push_native_mail_service(
        r#"{"kind":"cost","cost":125}"#.to_owned()
    ));
    app.update();
    let events = app
        .world_mut()
        .resource_mut::<mir2_client_bevy::mail_service::MailServiceInbox>()
        .drain();
    assert_eq!(
        events.last(),
        Some(&mir2_client_bevy::mail_service::MailServiceEvent::Cost { cost: 125 })
    );

    {
        let mut inbox = app
            .world_mut()
            .resource_mut::<mir2_client_bevy::mail_service::MailServiceInbox>();
        for unique_id in 0..mir2_client_bevy::mail_service::MAIL_SERVICE_INBOX_CAPACITY {
            assert!(inbox.push(
                mir2_client_bevy::mail_service::MailServiceEvent::LockedItem {
                    unique_id: unique_id as u64 + 101,
                    locked: true,
                },
            ));
        }
    }
    assert!(native_ingest::push_native_mail_service(
        r#"{"kind":"cost","cost":250}"#.to_owned()
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<mir2_client_bevy::mail_service::MailServiceInbox>()
            .len(),
        mir2_client_bevy::mail_service::MAIL_SERVICE_INBOX_CAPACITY + 1
    );
    assert!(native_ingest::push_native_data_reset());
    app.update();
    assert!(app
        .world()
        .resource::<mir2_client_bevy::mail_service::MailServiceInbox>()
        .is_empty());
}
