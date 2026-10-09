use super::*;

fn feedback_test_app() -> App {
    let mut app = App::new();
    app.insert_resource(LegacyMailSendFeedbackFixture).init_resource::<NativePlayerUiState>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<MailModel>()
        .init_resource::<MailComposeUi>()
        .init_resource::<ShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<crate::social::SocialModel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::audio::NativeUiAudioQueue>()
        .add_message::<KeyboardInput>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });
    super::tests::init_overlay_button_test_resources(&mut app);
    app.add_systems(
        Update,
        (
            consume_mail_operation_feedback,
            process_overlay_keyboard,
            process_overlay_buttons,
        )
            .chain(),
    );
    app
}

fn receipt(kind: MailOperationKind, success: bool, mail_id: Option<u64>) -> MailMessage {
    MailMessage {
        operation: Some(crate::mail::MailOperationFeedback {
            kind,
            success,
            mail_id,
        }),
        ..Default::default()
    }
}

fn draft() -> mir2_ui_core::state::MailComposeDraft {
    mir2_ui_core::state::MailComposeDraft {
        recipient: "Trader".to_owned(),
        message: "A letter".to_owned(),
        ..Default::default()
    }
}

fn press(app: &mut App, button: OverlayButton) {
    let entity = app
        .world_mut()
        .spawn((Interaction::Pressed, button, Button))
        .id();
    app.update();
    app.world_mut().despawn(entity);
}

fn mail_tracked_app()->App{
    let mut app=feedback_test_app();app.world_mut().remove_resource::<LegacyMailSendFeedbackFixture>();
    app.insert_resource(SessionResetRevision(1)).add_plugins(Mir2NativeMailParcelServicePlugin);
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().start_stream(crate::mail_service::MailServiceStreamStarted{epoch:MailServiceStreamEpoch{run:1,connection:1}}));
    {let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.apply(mir2_ui_core::action::UiAction::OpenMail);state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);state.apply(mir2_ui_core::action::UiAction::SetMailRecipient{recipient:" Trader ".into()});state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:" A letter\r\n ".into()});}
    app
}
/// Explicit entered fixture. This tests actual Submit/prebind/inbox consumer,
/// and does not claim to have called a socket or the platform gateway sink.
fn mail_bind_submitted_fixture(app:&mut App)->MailSendTicket{
    let mut queue=app.world_mut().remove_resource::<NativePlayerUiIntentQueue>().unwrap();
    let drained=queue.drain_for_gateway();assert!(drained.iter().any(|(intent,_)|matches!(intent,NativePlayerUiIntent::SendMail{..})));
    let token=queue.take_drained_mail_send_token().unwrap();
    let ticket=MailSendTicket{run:1,connection:1,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:74,local_send_token:token.value()};
    assert!(queue.bind_mail_send(token,ticket,&mut app.world_mut().resource_mut::<PendingOperations>()));app.insert_resource(queue);ticket
}
fn mail_enter_fixture(app:&mut App,ticket:MailSendTicket){
    let mut inbox=app.world_mut().resource_mut::<MailServiceInbox>();assert!(inbox.push_send_receipt(MailSendReceipt{ticket,outcome:MailSendOutcome::Entered}));assert!(inbox.push_send_receipt(MailSendReceipt{ticket,outcome:MailSendOutcome::Flushed}));drop(inbox);app.update();
}
fn mail_ack_fixture(app:&mut App,ticket:MailSendTicket,result:i32){assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_send_acknowledgement(MailSendAcknowledgement{ticket,result}));app.update();}

#[test]
fn mail_duplicate_actual_letter_submit_preserves_entered_capture_and_exact_binding_for_both_ack_results(){
    for needs_cleanup in [false,true]{for result in [1,-1]{
        let mut app=mail_tracked_app();
        if needs_cleanup{let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.apply(mir2_ui_core::action::UiAction::SetMailGold{gold:9});state.apply(mir2_ui_core::action::UiAction::AddMailAttachment{unique_id:41});}
        let before=app.world().resource::<NativePlayerUiState>().core.mail_compose.clone().unwrap();
        let generation=app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation().unwrap();
        press(&mut app,OverlayButton::SubmitMail);
        let mut submitted=before;submitted.gold=0;submitted.attachment_unique_ids.clear();
        let submitted_generation=app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation().unwrap();
        assert_eq!(submitted_generation,generation+u64::from(needs_cleanup),"only real Letter sanitization changes generation");
        assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref(),Some(&submitted));
        let ticket=mail_bind_submitted_fixture(&mut app);mail_enter_fixture(&mut app,ticket);
        let flight=app.world().resource::<NativePlayerUiIntentQueue>().mail_send.flight().unwrap().clone();
        assert!(flight.entered);assert_eq!(flight.value.ticket,Some(ticket));
        let key=NativePlayerUiIntent::SendMail{recipient:flight.value.payload.recipient.clone(),message:flight.value.payload.message.clone(),gold:flight.value.payload.gold,attachment_unique_ids:flight.value.payload.attachment_unique_ids.clone(),stamped:flight.value.draft.stamped}.pending_key().unwrap();
        assert!(app.world_mut().resource_mut::<PendingOperations>().try_begin(PendingOperationKey::StorageUnlock));
        press(&mut app,OverlayButton::SubmitMail);
        assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref(),Some(&submitted));
        assert_eq!(app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation(),Some(submitted_generation));
        let queue=app.world().resource::<NativePlayerUiIntentQueue>();assert_eq!(queue.mail_send.flight(),Some(&flight));assert!(queue.intents.is_empty(),"duplicate Submit cannot enqueue another Send");
        {let mut pending=app.world_mut().resource_mut::<PendingOperations>();assert!(pending.has_pending_mail_send());assert!(pending.contains(&key));assert_eq!(pending.len(),2);
            assert!(!pending.settle_native_mail_send(ticket.local_send_token,None),"the original exact ticket remains bound");
            let wrong=MailSendTicket{sequence:ticket.sequence+1,..ticket};assert!(!pending.settle_native_mail_send(ticket.local_send_token,Some(wrong)));}
        mail_ack_fixture(&mut app,ticket,result);
        assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert!(app.world().resource::<PendingOperations>().contains(&PendingOperationKey::StorageUnlock));assert_eq!(app.world().resource::<PendingOperations>().len(),1);
        assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token(),None);
        if result==1{assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());}
        else{assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref(),Some(&submitted));assert_eq!(app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation(),Some(submitted_generation));assert_eq!(app.world().resource::<MailComposeUi>().last_notice.as_deref(),Some("Mail was rejected; draft kept"));}
    }}
}

#[test]
fn mail_two_pressed_submit_buttons_in_one_update_keep_first_capture_generation_and_one_send(){
    for result in [1,-1]{
        let mut app=mail_tracked_app();let original=app.world().resource::<NativePlayerUiState>().core.mail_compose.clone();let generation=app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation();
        let first=app.world_mut().spawn((Interaction::Pressed,OverlayButton::SubmitMail,Button)).id();
        let second=app.world_mut().spawn((Interaction::Pressed,OverlayButton::SubmitMail,Button)).id();
        app.update();app.world_mut().despawn(first);app.world_mut().despawn(second);
        assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose,original);assert_eq!(app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation(),generation);
        let queue=app.world().resource::<NativePlayerUiIntentQueue>();assert_eq!(queue.intents.len(),1);assert_eq!(queue.intents.iter().filter(|intent|matches!(intent,NativePlayerUiIntent::SendMail{..})).count(),1);
        let token=queue.mail_send_token().unwrap();assert_eq!(Some(queue.mail_send.flight().unwrap().value.draft.generation),generation);assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<PendingOperations>().len(),1);
        let ticket=mail_bind_submitted_fixture(&mut app);assert_eq!(ticket.local_send_token,token.value());mail_enter_fixture(&mut app,ticket);
        assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket(),Some(ticket));assert_eq!(app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation(),generation);
        mail_ack_fixture(&mut app,ticket,result);
        assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token(),None);
        if result==1{assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());}
        else{assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose,original);assert_eq!(app.world().resource::<NativePlayerUiState>().mail_draft_clock.generation(),generation);}
    }
}

#[test]
fn mail_actual_submit_typed_ack_settles_old_pending_independently_of_full_draft_effect_gate(){
    for changed in [false,true]{for result in [1,-1]{
        let mut app=mail_tracked_app();let original=app.world().resource::<NativePlayerUiState>().core.mail_compose.clone();
        press(&mut app,OverlayButton::SubmitMail);let ticket=mail_bind_submitted_fixture(&mut app);mail_enter_fixture(&mut app,ticket);
        if changed{let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();
            state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:"B".into()});state.apply(mir2_ui_core::action::UiAction::SetMailGold{gold:9});state.mail_feedback_prompt=Some("new modal".into());}
        let before=app.world().resource::<NativePlayerUiState>().clone();let notice=app.world().resource::<MailComposeUi>().last_notice.clone();
        mail_ack_fixture(&mut app,ticket,result);
        assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token(),None);
        if changed{assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose,before.core.mail_compose);assert_eq!(app.world().resource::<NativePlayerUiState>().mail_feedback_prompt,before.mail_feedback_prompt);assert_eq!(app.world().resource::<MailComposeUi>().last_notice,notice);}
        else if result==1{assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());}
        else{assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose,original);assert_eq!(app.world().resource::<MailComposeUi>().last_notice.as_deref(),Some("Mail was rejected; draft kept"));}
    }}
}

#[test]
fn mail_real_reducer_aba_equal_fingerprint_and_generic_feedback_cannot_touch_new_binding(){
    let mut app=mail_tracked_app();press(&mut app,OverlayButton::SubmitMail);let old=mail_bind_submitted_fixture(&mut app);mail_enter_fixture(&mut app,old);
    {let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();let original=state.core.mail_compose.clone().unwrap();let generation=state.mail_draft_clock.generation();
        state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:"B".into()});state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:original.message.clone()});
        state.apply(mir2_ui_core::action::UiAction::SetMailRecipient{recipient:"Other".into()});state.apply(mir2_ui_core::action::UiAction::SetMailRecipient{recipient:original.recipient.clone()});
        state.apply(mir2_ui_core::action::UiAction::SetMailGold{gold:7});state.apply(mir2_ui_core::action::UiAction::SetMailGold{gold:original.gold});
        assert_eq!(state.core.mail_compose.as_ref(),Some(&original));assert_ne!(generation,state.mail_draft_clock.generation());}
    mail_ack_fixture(&mut app,old,1);assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_some());
    press(&mut app,OverlayButton::SubmitMail);let new=mail_bind_submitted_fixture(&mut app);assert_ne!(old.local_send_token,new.local_send_token);mail_enter_fixture(&mut app,new);
    let before=app.world().resource::<NativePlayerUiState>().clone();
    mail_ack_fixture(&mut app,old,1);assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket(),Some(new));assert_eq!(app.world().resource::<NativePlayerUiState>(),&before);
    app.world_mut().resource_mut::<MailModel>().mails=vec![receipt(MailOperationKind::Send,true,None)];app.update();assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<NativePlayerUiState>(),&before);
    mail_ack_fixture(&mut app,new,-1);assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());
}

#[test]
fn mail_prepublished_reset_retains_provisional_then_entered_tombstone_and_exact_unsent(){
    for entered in [false,true]{
        let mut app=mail_tracked_app();press(&mut app,OverlayButton::SubmitMail);let old=mail_bind_submitted_fixture(&mut app);
        {app.world_mut().resource_mut::<NativePlayerUiState>().reset_session();app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().clear();app.world_mut().resource_mut::<PendingOperations>().clear();app.world_mut().resource_mut::<SessionResetRevision>().0=2;}
        assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket(),Some(old));
        if entered{mail_enter_fixture(&mut app,old);}else{assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_send_receipt(MailSendReceipt{ticket:old,outcome:MailSendOutcome::DefinitelyUnsent}));app.update();}
        if entered{assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());mail_ack_fixture(&mut app,old,1);}
        assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());
    }
}
#[test]
fn mail_actual_submit_generation_overflow_stays_fail_closed_across_reset_and_new_stream(){
    let mut app=mail_tracked_app();
    {let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.mail_draft_clock=MailDraftClock::from_highwater(u64::MAX);state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:"B".into()});assert_eq!(state.mail_draft_clock.generation(),None);}
    press(&mut app,OverlayButton::SubmitMail);assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<MailComposeUi>().last_notice.as_deref(),Some("Mail draft authorization unavailable"));
    {let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.reset_session();state.apply(mir2_ui_core::action::UiAction::OpenMail);state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);state.apply(mir2_ui_core::action::UiAction::SetMailRecipient{recipient:"R".into()});state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:"B".into()});assert_eq!(state.mail_draft_clock.generation(),None);}
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().start_stream(crate::mail_service::MailServiceStreamStarted{epoch:MailServiceStreamEpoch{run:1,connection:2}}));
    press(&mut app,OverlayButton::SubmitMail);assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_some());
}
#[test]
fn mail_send_sidecar_token_follows_exact_admission_position_and_fifo_eviction_cancels_only_it(){
    let mut app=mail_tracked_app();let unrelated=NativePlayerUiIntent::SendMail{recipient:"other".into(),message:"other".into(),gold:0,attachment_unique_ids:vec![],stamped:false};
    assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().push_intent(unrelated));press(&mut app,OverlayButton::SubmitMail);
    let old=app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token().unwrap();let drained=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_for_gateway_with_send();assert_eq!(drained.len(),2);assert_eq!(drained[0].2,None);assert_eq!(drained[1].2,Some(old));
    assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().reject_unpublished_mail_send(old));app.update();assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());
    press(&mut app,OverlayButton::SubmitMail);let next=app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token().unwrap();assert_ne!(old,next);
    {let mut queue=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>();for id in 1..=MAX_QUEUED as u64{assert!(queue.push_transient_unique(NativePlayerUiIntent::MailLockItem{unique_id:1000+id,locked:true}));}}
    app.update();assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token(),None);assert!(!app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().reject_unpublished_mail_send(old));
}

#[test]
fn feedback_modal_renders_source_frame_message_and_ok_above_mail_layers() {
    let mut app = super::tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .mail_feedback_prompt = Some("Recipient and message are required".to_owned());
    app.update();

    let world = app.world_mut();
    let (node, focus, z_index) = world
        .query_filtered::<(&Node, &FocusPolicy, &GlobalZIndex), With<OverlayMailFeedbackModal>>()
        .single(world)
        .expect("mail feedback modal root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(*focus, FocusPolicy::Block);
    assert_eq!(*z_index, GlobalZIndex(OVERLAY_MAIL_FEEDBACK_MODAL_Z));
    assert!(world
        .query::<&OverlayMailFeedbackDialog>()
        .iter(world)
        .next()
        .is_some());
    assert!(world
        .query::<&Text>()
        .iter(world)
        .any(|text| text.0 == "Recipient and message are required"));
    assert!(world.query::<&OverlayButton>().iter(world).any(|button| {
        matches!(button, OverlayButton::MailFeedbackAcknowledge)
    }));
}

#[test]
fn result_receipts_close_only_successful_source_surfaces_and_keep_rejected_draft() {
    let mut app = feedback_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .mail_compose = Some(draft());
    app.world_mut()
        .resource_mut::<MailModel>()
        .mails
        .push(receipt(MailOperationKind::Send, false, None));
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .core
            .mail_compose,
        Some(draft()),
        "a rejected result preserves the authoritative draft for retry"
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .mail_feedback_prompt
            .as_deref(),
        Some("Mail was rejected; draft kept")
    );

    app.world_mut()
        .resource_mut::<MailModel>()
        .mails
        .push(receipt(MailOperationKind::Send, true, None));
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .core
        .mail_compose
        .is_none(), "successful MailSent closes the compose surface without a success modal");
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_feedback_prompt
        .is_none());

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 44,
        kind: MailReaderKind::Parcel,
    });
    app.world_mut()
        .resource_mut::<MailModel>()
        .mails
        .push(receipt(MailOperationKind::Collect, true, Some(44)));
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_reader
        .is_none(), "a successful parcel collection closes only its matching reader");

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 45,
        kind: MailReaderKind::Parcel,
    });
    app.world_mut()
        .resource_mut::<MailModel>()
        .mails
        .push(receipt(MailOperationKind::Collect, false, Some(45)));
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_reader
        .is_some(), "a failed collection keeps its reader visible behind the error box");
    press(&mut app, OverlayButton::MailFeedbackAcknowledge);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_feedback_prompt
        .is_none(), "feedback OK remains reachable above an open reader");
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_reader
        .is_some());
}

#[test]
fn local_validation_and_feedback_close_consume_covered_mail_actions() {
    let mut app = feedback_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .mail_compose = Some(mir2_ui_core::state::MailComposeDraft::default());

    press(&mut app, OverlayButton::SubmitMail);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .mail_feedback_prompt
            .as_deref(),
        Some("Recipient and message are required")
    );
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .core
        .mail_compose
        .is_some(), "local validation must retain an editable draft");

    let cancel = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::CancelMailCompose, Button))
        .id();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();
    app.world_mut().despawn(cancel);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_feedback_prompt
        .is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_feedback_input_consumed);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .core
        .mail_compose
        .is_some(), "Enter closing a notice cannot also cancel the covered draft");

    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.mail_feedback_prompt = Some("Mail claim failed".to_owned());
        state.mail_feedback_input_consumed = false;
    }
    let acknowledge = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::MailFeedbackAcknowledge, Button))
        .id();
    let covered = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::CancelMailCompose, Button))
        .id();
    app.update();
    app.world_mut().despawn(acknowledge);
    app.world_mut().despawn(covered);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_feedback_input_consumed);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .core
        .mail_compose
        .is_some(), "the OK close frame cannot reach a covered mail action");

    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.mail_feedback_prompt = Some("Mail delete failed".to_owned());
    state.close_windows();
    assert!(state.mail_feedback_prompt.is_none());
    state.mail_feedback_prompt = Some("Mail read failed".to_owned());
    state.reset_session();
    assert!(state.mail_feedback_prompt.is_none());
}
