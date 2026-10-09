use super::*;
use mir2_client_bevy::mail_service::{MailServiceDelivery, MailServiceEvent, MailServiceInbox, MailServiceInboxMessage, MailServiceStreamEpoch, MailServiceStreamStarted, MAIL_SERVICE_INBOX_CAPACITY};

const EPOCH: MailServiceStreamEpoch = MailServiceStreamEpoch { run: 1, connection: 1 };
fn quote_receipt()->mir2_client_bevy::mail_service::MailQuoteReceipt {
    use mir2_client_bevy::mail_service::{MailQuoteReceipt,MailQuoteTicket,MailQuoteOutcome};
    MailQuoteReceipt{ticket:MailQuoteTicket{run:1,connection:1,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:73,local_quote_token:19},outcome:MailQuoteOutcome::Entered{at_ms:500}}
}
fn mail_send_receipt(outcome:mir2_client_bevy::mail_service::MailSendOutcome)->mir2_client_bevy::mail_service::MailSendReceipt{
    use mir2_client_bevy::mail_service::{MailSendReceipt,MailSendTicket};MailSendReceipt{ticket:MailSendTicket{run:1,connection:1,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:74,local_send_token:20},outcome}
}
#[test]
fn native_mail_send_real_buffer_inbox_terminal_order_survives_session_and_scene_resets(){
    use mir2_client_bevy::mail_service::{MailSendOutcome,MailSendAcknowledgement};
    let _guard=native_ingest::native_queue_test_guard();
    for already_drained in [false,true]{for saturated_inbox in [false,true]{for branch in 0..3{
        let mut app=app();announce(EPOCH);
        if saturated_inbox{app.update();let mut inbox=app.world_mut().resource_mut::<MailServiceInbox>();for _ in 0..MAIL_SERVICE_INBOX_CAPACITY{assert!(inbox.push_delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::OpenParcel}));}}
        for index in 0..256{assert!(native_ingest::push_native_social_model(index.to_string()));}
        let quote=quote_receipt();assert!(native_ingest::push_native_mail_quote_receipt(quote));deliver(MailServiceEvent::Cost{cost:70});
        let entry=mail_send_receipt(MailSendOutcome::Entered);let write=mail_send_receipt(MailSendOutcome::Flushed);let ack=MailSendAcknowledgement{ticket:entry.ticket,result:1};
        assert!(native_ingest::push_native_mail_send_receipt(entry));assert!(native_ingest::push_native_mail_send_receipt(write));assert!(native_ingest::push_native_mail_send_acknowledgement(ack));
        if already_drained{app.update();}
        match branch{0=>assert!(native_ingest::push_native_data_reset()),1=>assert!(native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt(receipt())),_=>{
            // Scene boundaries preserve the personal mail stream as well.
            // An independent native_ingest test isolates stale-discard by
            // injecting each DataReset barrier after accepted terminals.
            assert!(native_ingest::push_native_scene_reset());
        }}
        app.update();
        let messages=app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered();
        let terminals=messages.into_iter().filter(|message|!matches!(message,MailServiceInboxMessage::StreamStarted(_)|MailServiceInboxMessage::Delivery(MailServiceDelivery{event:MailServiceEvent::OpenParcel|MailServiceEvent::LockedItem{..},..}))).collect::<Vec<_>>();
        assert_eq!(terminals,vec![MailServiceInboxMessage::QuoteReceipt(quote),MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::Cost{cost:70}}),MailServiceInboxMessage::SendReceipt(entry),MailServiceInboxMessage::SendReceipt(write),MailServiceInboxMessage::SendAcknowledgement(ack)]);
        assert!(!app.world().resource::<MailServiceInbox>().is_stream_failed());
    }}}
}
#[test]
fn native_mail_quote_receipt_and_cost_real_buffer_inbox_resets_preserve_fifo(){
    let _guard=native_ingest::native_queue_test_guard();
    for already_drained in [false,true]{for saturated_inbox in [false,true]{for preserving_shop in [false,true]{
        let mut app=app();announce(EPOCH);
        if saturated_inbox{app.update();let mut inbox=app.world_mut().resource_mut::<MailServiceInbox>();for _ in 0..MAIL_SERVICE_INBOX_CAPACITY{assert!(inbox.push_delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::LockedItem{unique_id:7,locked:true}}));}}
        deliver(MailServiceEvent::OpenParcel);deliver(MailServiceEvent::LockedItem{unique_id:7,locked:true});
        for index in 2..256{assert!(native_ingest::push_native_social_model(index.to_string()));}
        let quote=quote_receipt();assert!(native_ingest::push_native_mail_quote_receipt(quote));deliver(MailServiceEvent::Cost{cost:70});
        if already_drained{app.update();}
        if preserving_shop{assert!(native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt(receipt()));}else{assert!(native_ingest::push_native_data_reset());}
        app.update();assert_eq!(app.world().resource::<SessionResetRevision>().0,1);
        let messages=app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered();
        assert_eq!(messages,vec![MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted{epoch:EPOCH}),MailServiceInboxMessage::QuoteReceipt(quote),MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::Cost{cost:70}})]);
    }}}
}
#[test]
fn native_mail_quote_failed_before_first_ingest_marker_stays_failed(){
    let _guard=native_ingest::native_queue_test_guard();let mut app=app();announce(EPOCH);
    deliver(MailServiceEvent::Cost{cost:70});assert!(!native_ingest::push_native_mail_quote_receipt(quote_receipt()));app.update();
    assert!(app.world().resource::<MailServiceInbox>().is_stream_failed());announce(EPOCH);app.update();assert!(app.world().resource::<MailServiceInbox>().is_stream_failed());
    announce(MailServiceStreamEpoch{run:1,connection:2});app.update();assert!(!app.world().resource::<MailServiceInbox>().is_stream_failed());
}
#[test]
fn native_mail_quote_inbox_critical_refusal_marks_real_stream_failed(){
    let _guard=native_ingest::native_queue_test_guard();let mut app=app();announce(EPOCH);app.update();
    {let mut inbox=app.world_mut().resource_mut::<MailServiceInbox>();for _ in 0..MAIL_SERVICE_INBOX_CAPACITY{assert!(inbox.push_delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::OpenParcel}));}assert!(inbox.push_delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::Cost{cost:70}}));}
    assert!(native_ingest::push_native_mail_quote_receipt(quote_receipt()));app.update();
    assert!(app.world().resource::<MailServiceInbox>().is_stream_failed());assert_eq!(app.world().resource::<native_ingest::NativeInbound>().mail_stream_failure(),Some(EPOCH));
    assert!(native_ingest::push_native_data_reset());app.update();assert!(app.world().resource::<MailServiceInbox>().is_stream_failed());
    announce(MailServiceStreamEpoch{run:1,connection:2});app.update();assert!(!app.world().resource::<MailServiceInbox>().is_stream_failed());
}
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((Mir2NativeSessionBoundaryPlugin, Mir2NativeMailServiceIngestPlugin));
    app
}
fn announce(epoch: MailServiceStreamEpoch) {
    assert!(native_ingest::push_native_mail_service_stream_started(MailServiceStreamStarted { epoch }));
}
fn deliver(event: MailServiceEvent) {
    assert!(native_ingest::push_native_mail_service(MailServiceDelivery { epoch: EPOCH, event }));
}
fn receipt() -> mir2_client_bevy::game_shop::GameShopReceipt {
    serde_json::from_str(r#"{"protocol":"nativeGameShopReceiptV1","requestId":"gs-mail-reset","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"}"#).unwrap()
}

#[test]
fn mail_service_events_are_ordered_and_owner_events_clear_on_session_reset() {
    let _guard = native_ingest::native_queue_test_guard(); let mut app = app();
    announce(EPOCH); deliver(MailServiceEvent::OpenParcel); deliver(MailServiceEvent::Cost { cost: 125 });
    app.update();
    assert_eq!(app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered(), vec![
        MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: EPOCH }),
        MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: EPOCH, event: MailServiceEvent::OpenParcel }),
        MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: EPOCH, event: MailServiceEvent::Cost { cost: 125 } }),
    ]);
    deliver(MailServiceEvent::LockedItem { unique_id: 77, locked: true }); app.update();
    assert_eq!(app.world().resource::<MailServiceInbox>().len(), 1);
    assert!(native_ingest::push_native_data_reset()); app.update();
    assert!(app.world().resource::<MailServiceInbox>().is_empty());
    assert_eq!(app.world().resource::<MailServiceInbox>().stream_epoch(), Some(EPOCH));
}

#[test]
fn full_ui_fifo_still_delivers_single_flight_cost_and_same_stream_reset_retains_it() {
    let _guard = native_ingest::native_queue_test_guard(); let mut app = app();
    announce(EPOCH); app.update(); app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered();
    for cost in [125, 250] {
        {
            let mut inbox = app.world_mut().resource_mut::<MailServiceInbox>();
            for unique_id in 0..MAIL_SERVICE_INBOX_CAPACITY { assert!(inbox.push_delivery(MailServiceDelivery {
                epoch: EPOCH, event: MailServiceEvent::LockedItem { unique_id: unique_id as u64 + 1, locked: true },
            })); }
        }
        deliver(MailServiceEvent::Cost { cost }); app.update();
        assert_eq!(app.world().resource::<MailServiceInbox>().len(), MAIL_SERVICE_INBOX_CAPACITY + 1);
        if cost == 125 {
            let events = app.world_mut().resource_mut::<MailServiceInbox>().drain();
            assert_eq!(events.last(), Some(&MailServiceEvent::Cost { cost }));
        } else {
            // Intentional reset semantics: already-drained current-stream Cost
            // survives while all ordinary owner lock echoes still disappear.
            assert!(native_ingest::push_native_data_reset()); app.update();
            assert_eq!(app.world_mut().resource_mut::<MailServiceInbox>().drain(), vec![MailServiceEvent::Cost { cost }]);
        }
    }
}

#[test]
fn queued_and_already_drained_cost_survive_both_runtime_session_reset_branches() {
    let _guard = native_ingest::native_queue_test_guard();
    for already_drained in [false, true] { for preserving_shop in [false, true] {
        let mut app = app(); announce(EPOCH);
        deliver(MailServiceEvent::OpenParcel); deliver(MailServiceEvent::LockedItem { unique_id: 77, locked: true });
        deliver(MailServiceEvent::Cost { cost: 125 });
        if already_drained { app.update(); }
        if preserving_shop { assert!(native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt(receipt())); }
        else { assert!(native_ingest::push_native_data_reset()); }
        app.update();
        assert_eq!(app.world().resource::<SessionResetRevision>().0, 1);
        assert_eq!(app.world().resource::<MailServiceInbox>().stream_epoch(), Some(EPOCH));
        assert_eq!(app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered(), vec![
            MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: EPOCH }),
            MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: EPOCH, event: MailServiceEvent::Cost { cost: 125 } }),
        ]);
    }}
}

#[test]
fn newer_native_stream_clears_old_inbox_and_reserve_while_same_marker_is_idempotent() {
    let _guard = native_ingest::native_queue_test_guard(); let mut app = app();
    announce(EPOCH); app.update(); app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered();
    {
        let mut inbox = app.world_mut().resource_mut::<MailServiceInbox>();
        for _ in 0..MAIL_SERVICE_INBOX_CAPACITY { assert!(inbox.push_delivery(MailServiceDelivery { epoch: EPOCH, event: MailServiceEvent::OpenParcel })); }
    }
    deliver(MailServiceEvent::Cost { cost: 125 }); app.update();
    announce(EPOCH); app.update();
    assert_eq!(app.world().resource::<MailServiceInbox>().len(), MAIL_SERVICE_INBOX_CAPACITY + 1);
    let newer = MailServiceStreamEpoch { run: 2, connection: 1 };
    announce(newer);
    assert!(!native_ingest::push_native_mail_service(MailServiceDelivery { epoch: EPOCH, event: MailServiceEvent::Cost { cost: 125 } }));
    assert!(native_ingest::push_native_mail_service(MailServiceDelivery { epoch: newer, event: MailServiceEvent::Cost { cost: 250 } }));
    app.update();
    assert_eq!(app.world_mut().resource_mut::<MailServiceInbox>().drain_ordered(), vec![
        MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: newer }),
        MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: newer, event: MailServiceEvent::Cost { cost: 250 } }),
    ]);
}

#[test]
fn native_scene_reset_preserves_entire_current_mail_stream() {
    let _guard = native_ingest::native_queue_test_guard(); let mut app = app();
    announce(EPOCH); deliver(MailServiceEvent::OpenParcel); deliver(MailServiceEvent::Cost { cost: 125 });
    assert!(native_ingest::push_native_scene_reset()); app.update();
    assert_eq!(app.world().resource::<SessionResetRevision>().0, 0);
    assert_eq!(app.world_mut().resource_mut::<MailServiceInbox>().drain(), vec![MailServiceEvent::OpenParcel, MailServiceEvent::Cost { cost: 125 }]);
}
