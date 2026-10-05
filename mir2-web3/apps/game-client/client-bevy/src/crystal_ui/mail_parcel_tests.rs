use super::*;

const TEST_MAIL_EPOCH: MailServiceStreamEpoch = MailServiceStreamEpoch { run: 1, connection: 1 };
#[test]
fn mail_full_draft_ordered_uid_stamp_and_incarnation_aba_bump_synchronously(){
    let mut state=NativePlayerUiState::default();state.apply(mir2_ui_core::action::UiAction::OpenMail);state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);
    let mut parcel=mail_parcel::MailParcelUi::default();parcel.bind_draft_clock(state.mail_draft_clock.clone());parcel.observe_stream(Some(TEST_MAIL_EPOCH));
    let mut stamp=source_item(838,2);let source=stamp.tooltip_source.as_mut().unwrap();source.info.item_index=838;source.user_item.as_mut().unwrap().item_index=838;source.info.shape=1;source.info.item_type=0;
    let inventory=InventoryModel{items:vec![source_item(41,0),source_item(42,1),stamp],..default()};
    assert_eq!(parcel.slot_limit(),1);assert!(parcel.stamp_available(&inventory));
    assert!(parcel.toggle_stamp(&inventory));assert!(parcel.stamped());assert_eq!(parcel.slot_limit(),5);
    assert!(parcel.attach(state.core.mail_compose.as_mut().unwrap(),&inventory,41));assert!(parcel.attach(state.core.mail_compose.as_mut().unwrap(),&inventory,42));
    let original=state.core.mail_compose.clone();let generation=state.mail_draft_clock.generation();
    assert_eq!(parcel.detach_at(state.core.mail_compose.as_mut().unwrap(),0),Some(41));assert!(parcel.attach(state.core.mail_compose.as_mut().unwrap(),&inventory,41));
    assert_eq!(parcel.detach_at(state.core.mail_compose.as_mut().unwrap(),0),Some(42));assert!(parcel.attach(state.core.mail_compose.as_mut().unwrap(),&inventory,42));assert_eq!(state.core.mail_compose,original);assert_ne!(state.mail_draft_clock.generation(),generation);
    assert_eq!(parcel.detach_at(state.core.mail_compose.as_mut().unwrap(),1),Some(42));
    assert!(parcel.toggle_stamp(&inventory));assert!(!parcel.stamped());assert_eq!(parcel.slot_limit(),1);
    let before_stamp=state.mail_draft_clock.generation();assert!(parcel.toggle_stamp(&inventory));let stamped=state.mail_draft_clock.generation();assert_ne!(before_stamp,stamped);assert!(parcel.toggle_stamp(&inventory));assert!(!parcel.stamped());assert_ne!(state.mail_draft_clock.generation(),before_stamp);
    let before_close=state.mail_draft_clock.generation();state.close_all_windows();assert_ne!(state.mail_draft_clock.generation(),before_close);let retained_clock=state.mail_draft_clock.clone();let before_reset=state.mail_draft_clock.generation();state.reset_session();assert!(state.mail_draft_clock.generation()>before_reset);assert_eq!(retained_clock.generation(),state.mail_draft_clock.generation());
}
fn announce_mail_stream(app:&mut App) {
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().start_stream(
        crate::mail_service::MailServiceStreamStarted { epoch: TEST_MAIL_EPOCH }));
}
fn fixture_ticket(token:NativeQuoteToken,epoch:MailServiceStreamEpoch)->MailQuoteTicket {
    MailQuoteTicket{run:epoch.run,connection:epoch.connection,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:token.value(),local_quote_token:token.value()}
}
/// Explicit controlled entry publisher for these UI-only fixtures. Production
/// obtains the stamp/sequence from GatewayCommandSender before publication.
fn drain_entered_mail_fixture(app:&mut App)->Vec<NativePlayerUiIntent>{
    let epoch=app.world().resource::<MailServiceInbox>().stream_epoch();
    let drained=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_for_gateway();
    drained.into_iter().map(|(intent,token)|{
        if let Some(token)=token{
            let ticket=fixture_ticket(token,epoch.expect("fixture stream required"));assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().bind_mail_quote(token,ticket));
            assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_quote_receipt(MailQuoteReceipt{ticket,outcome:MailQuoteOutcome::Entered{at_ms:crate::hero_model::hero_clock_ms()}}));
        }intent
    }).collect()
}

fn tracked_mail_app()->App {
    let mut app=App::new();app.add_plugins(Mir2NativeMailParcelServicePlugin);app.init_resource::<SessionResetRevision>();
    app.world_mut().resource_mut::<NativeShellModel>().screen=NativeShellScreen::InGame;
    {let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.core.panel=mir2_ui_core::state::UiPanel::Mail;state.core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{gold:700,..default()});}
    app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;announce_mail_stream(&mut app);app.update();app
}
#[test]
fn mail_typed_send_ack_unlocks_captured_ids_only_for_matching_full_parcel_draft(){
    for changed in [false,true]{for result in [1,-1]{
        let epoch=TEST_MAIL_EPOCH;let mut app=App::new();app.add_plugins(Mir2NativeMailParcelServicePlugin).insert_resource(SessionResetRevision(1));
        app.world_mut().resource_mut::<NativeShellModel>().screen=NativeShellScreen::InGame;
        app.insert_resource(InventoryModel{items:vec![source_item(41,0),source_item(42,1)],..default()});
        announce_mail_stream(&mut app);app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;
        let mut state=app.world_mut().remove_resource::<NativePlayerUiState>().unwrap();state.apply(mir2_ui_core::action::UiAction::OpenMail);state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);state.apply(mir2_ui_core::action::UiAction::SetMailRecipient{recipient:" R ".into()});state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:" A\r\n ".into()});
        {let inventory=app.world().resource::<InventoryModel>().clone();let mut parcel=app.world_mut().resource_mut::<mail_parcel::MailParcelUi>();parcel.bind_draft_clock(state.mail_draft_clock.clone());parcel.observe_stream(Some(epoch));assert!(parcel.attach(state.core.mail_compose.as_mut().unwrap(),&inventory,41));}
        let snapshot=native_mail_send_draft(&state,app.world().resource::<MailComposeUi>(),app.world().resource::<mail_parcel::MailParcelUi>()).unwrap();
        let payload=mir2_client_core::mail_compose::prepare_send(&snapshot.recipient,&snapshot.message,snapshot.gold,&snapshot.attachment_unique_ids).unwrap();
        app.insert_resource(state);let mut queue=app.world_mut().remove_resource::<NativePlayerUiIntentQueue>().unwrap();
        let intent=NativePlayerUiIntent::SendMail{recipient:payload.recipient.clone(),message:payload.message.clone(),gold:payload.gold,attachment_unique_ids:payload.attachment_unique_ids.clone(),stamped:snapshot.stamped};
        assert!(queue.push_mail_send(&mut app.world_mut().resource_mut::<PendingOperations>(),epoch,1,snapshot,payload,intent));let drained=queue.drain_for_gateway_with_send();let token=drained[0].2.unwrap();
        let ticket=MailSendTicket{run:1,connection:1,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:74,local_send_token:token.value()};
        assert!(queue.bind_mail_send(token,ticket,&mut app.world_mut().resource_mut::<PendingOperations>()));app.insert_resource(queue);
        assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_send_receipt(MailSendReceipt{ticket,outcome:MailSendOutcome::Entered}));app.update();
        if changed{let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.apply(mir2_ui_core::action::UiAction::RemoveMailAttachment{unique_id:41});state.apply(mir2_ui_core::action::UiAction::AddMailAttachment{unique_id:42});state.apply(mir2_ui_core::action::UiAction::SetMailMessage{message:"B".into()});drop(state);app.update();}
        let before=app.world().resource::<NativePlayerUiState>().core.mail_compose.clone();
        assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_send_acknowledgement(MailSendAcknowledgement{ticket,result}));app.update();
        assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());let parcel=app.world().resource::<mail_parcel::MailParcelUi>();
        if changed{assert!(parcel.blocks_item(42));assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose,before);}
        else if result==1{assert!(!parcel.blocks_item(41));assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());let unlocked=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents();assert!(unlocked.iter().any(|intent|matches!(intent,NativePlayerUiIntent::MailLockItem{unique_id:41,locked:false})));assert!(!unlocked.iter().any(|intent|matches!(intent,NativePlayerUiIntent::MailLockItem{unique_id:42,locked:false})));}
        else{assert!(parcel.blocks_item(41));assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose,before);}
    }}
}
#[test]
fn native_mail_unpublished_fifo_eviction_cancels_exact_token(){
    let mut app=tracked_mail_app();let old=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();
    {let mut queue=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>();for id in 1..=MAX_QUEUED as u64{assert!(queue.push_transient_unique(NativePlayerUiIntent::MailLockItem{unique_id:1000+id,locked:true}));}
    assert!(!queue.intents.iter().any(|intent|matches!(intent,NativePlayerUiIntent::MailCost{..})));assert_eq!(queue.mail_quote_token(),Some(old));}
    app.update();let next=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();assert_ne!(next,old);
    assert!(!app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().reject_unpublished_mail_quote(old));
}
#[test]
fn native_mail_provisional_queue_clear_and_old_receipt_cannot_cancel_same_draft_successor(){
    let mut app=tracked_mail_app();let old=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();
    app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().clear();app.update();
    let next=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();assert_ne!(old,next);
    let old_ticket=fixture_ticket(old,TEST_MAIL_EPOCH);
    {let mut queue=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>();assert!(!queue.bind_mail_quote(old,old_ticket));assert!(!queue.reject_unpublished_mail_quote(old));}
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_quote_receipt(MailQuoteReceipt{ticket:old_ticket,outcome:MailQuoteOutcome::DefinitelyUnsent}));app.update();
    assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token(),Some(next));assert!(app.world().resource::<mail_parcel::MailParcelUi>().has_pending_quote());
}
#[test]
fn native_mail_published_provisional_survives_clear_reset_and_late_entry_before_cost(){
    let mut app=tracked_mail_app();let drained=app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_for_gateway();let token=drained[0].1.unwrap();let ticket=fixture_ticket(token,TEST_MAIL_EPOCH);
    assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().bind_mail_quote(token,ticket));
    app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().clear();app.world_mut().resource_mut::<SessionResetRevision>().0=1;app.update();app.update();
    assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token(),Some(token));
    app.world_mut().resource_mut::<mail_parcel::MailParcelUi>().tick_quote_timeout(u64::MAX);assert_eq!(app.world().resource::<mail_parcel::MailParcelUi>().quote_error(),None);
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_quote_receipt(MailQuoteReceipt{ticket,outcome:MailQuoteOutcome::Entered{at_ms:100}}));
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push(MailServiceEvent::Cost{cost:70}));app.update();
    let next=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();assert_ne!(token,next);
    let draft=app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap();assert!(!app.world().resource::<mail_parcel::MailParcelUi>().quote_is_current(draft,app.world().resource::<InventoryModel>()));
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_quote_receipt(MailQuoteReceipt{ticket,outcome:MailQuoteOutcome::DefinitelyUnsent}));app.update();assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token(),Some(next));
}
#[test]
fn native_mail_true_new_stream_discards_provisional_and_failed_stream_cannot_quote(){
    let mut app=tracked_mail_app();let old=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();
    app.world_mut().resource_mut::<MailServiceInbox>().fail_stream(TEST_MAIL_EPOCH);app.update();
    assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token(),Some(old));
    let newer=MailServiceStreamEpoch{run:TEST_MAIL_EPOCH.run,connection:TEST_MAIL_EPOCH.connection+1};assert!(app.world_mut().resource_mut::<MailServiceInbox>().start_stream(crate::mail_service::MailServiceStreamStarted{epoch:newer}));app.update();
    let next=app.world().resource::<NativePlayerUiIntentQueue>().mail_quote_token().unwrap();assert_ne!(old,next);
    assert!(!app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().bind_mail_quote(old,fixture_ticket(old,TEST_MAIL_EPOCH)));
}

#[test]
fn actual_native_quote_queue_rejection_releases_only_unsubmitted_reservation() {
    let mut app=App::new();
    app.init_resource::<NativePlayerUiState>().init_resource::<MailComposeUi>()
        .init_resource::<InventoryModel>().init_resource::<mail_parcel::MailParcelUi>()
        .init_resource::<PendingOperations>().init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<MailServiceInbox>()
        .insert_resource(NativeShellModel {screen:NativeShellScreen::InGame,..default()})
        .add_systems(Update,sync_mail_parcel_quote);
    announce_mail_stream(&mut app);
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel=mir2_ui_core::state::UiPanel::Mail;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{gold:700,..default()});
    app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;
    for _ in 0..24 {assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().push_intent(NativePlayerUiIntent::RefreshFriends));}
    app.update();
    assert_eq!(app.world().resource::<mail_parcel::MailParcelUi>().quote_error(),Some("Postage request was not sent"));
    drain_entered_mail_fixture(&mut app);app.update();
    let sent=drain_entered_mail_fixture(&mut app);
    assert_eq!(sent,vec![NativePlayerUiIntent::MailCost{gold:700,attachment_unique_ids:vec![],stamped:false}]);
    app.update();assert!(drain_entered_mail_fixture(&mut app).is_empty(),"an already submitted flight cannot be canceled by a later no-op");
}

#[test]
fn native_shared_quote_rejects_equal_weight_but_changed_stat_tuples() {
    let mut ui=mail_parcel::MailParcelUi::default();let mut item=source_item(41,40);
    item.tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().added_stats=vec![crate::inventory::CrystalItemStatModel{stat:1,value:1},crate::inventory::CrystalItemStatModel{stat:2,value:-1}];
    let mut inv=InventoryModel{capacity:86,items:vec![item],..default()};
    let draft=mir2_ui_core::state::MailComposeDraft{attachment_unique_ids:vec![41],..default()};
    assert!(ui.begin_quote(&draft,&inv,1).is_some());
    inv.items[0].tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().added_stats=vec![crate::inventory::CrystalItemStatModel{stat:1,value:2}];
    ui.apply_cost(5,Some(&draft),&inv);assert!(!ui.quote_is_current(&draft,&inv));
    assert!(ui.begin_quote(&draft,&inv,2).is_some());ui.apply_cost(5,Some(&draft),&inv);assert!(ui.quote_is_current(&draft,&inv));
}

#[test]
fn actual_native_same_connection_reset_and_close_retire_old_gold_quote() {
    let mut app=App::new();
    app.init_resource::<NativePlayerUiState>().init_resource::<MailComposeUi>()
        .init_resource::<InventoryModel>().init_resource::<mail_parcel::MailParcelUi>()
        .init_resource::<PendingOperations>().init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<MailServiceInbox>().init_resource::<SessionResetRevision>()
        .insert_resource(NativeShellModel{screen:NativeShellScreen::InGame,..default()})
        .add_systems(Update,(process_mail_service_inbox,sync_mail_parcel_quote).chain());
    announce_mail_stream(&mut app);
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel=mir2_ui_core::state::UiPanel::Mail;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{gold:700,..default()});
    app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;
    app.update();assert_eq!(drain_entered_mail_fixture(&mut app).len(),1);
    // A logout/character reset may retain the socket; equal new gold cannot
    // reuse the previous uncorrelated response slot.
    app.world_mut().resource_mut::<SessionResetRevision>().0+=1;app.update();app.update();
    assert!(drain_entered_mail_fixture(&mut app).is_empty());
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push(MailServiceEvent::Cost{cost:70}));app.update();
    let draft=app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap();
    assert!(!app.world().resource::<mail_parcel::MailParcelUi>().quote_is_current(draft,app.world().resource::<InventoryModel>()));
    assert_eq!(drain_entered_mail_fixture(&mut app).len(),1,"old reply retires its slot and permits exactly one fresh quote");
    app.world_mut().resource_mut::<SessionResetRevision>().0+=1;
    app.world_mut().resource_mut::<MailServiceInbox>().push(MailServiceEvent::Cost{cost:70});app.update();
    let draft=app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap();
    assert!(!app.world().resource::<mail_parcel::MailParcelUi>().quote_is_current(draft,app.world().resource::<InventoryModel>()),"reset must be observed before a same-frame old Cost");
    assert_eq!(drain_entered_mail_fixture(&mut app).len(),1);
    // Gold-only close must also change the incarnation despite no selected UID.
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel=mir2_ui_core::state::UiPanel::None;app.update();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel=mir2_ui_core::state::UiPanel::Mail;app.update();
    assert!(drain_entered_mail_fixture(&mut app).is_empty());
    app.world_mut().resource_mut::<MailServiceInbox>().push(MailServiceEvent::Cost{cost:70});app.update();
    let draft=app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap();
    assert!(!app.world().resource::<mail_parcel::MailParcelUi>().quote_is_current(draft,app.world().resource::<InventoryModel>()));
}

fn source_item(unique_id: u64, slot: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(unique_id),
        slot,
        container: 0,
        quantity: 1,
        icon: 7,
        tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
            info: crate::inventory::CrystalItemInfoModel {
                item_index: 77,
                stack_size: 1,
                image: 7,
                ..default()
            },
            user_item: Some(crate::inventory::CrystalUserItemModel {
                unique_id,
                item_index: 77,
                count: 1,
                ..default()
            }),
            ..default()
        }),
        ..default()
    }
}

#[test]
fn parcel_renderer_uses_independent_source_frame_cells_and_bag_offset() {
    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.mail_inventory_visible = true;
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "first\nsecond".into(),
            attachment_unique_ids: vec![41],
            ..default()
        });
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;
    app.world_mut().resource_mut::<InventoryModel>().items = vec![source_item(41, 0), source_item(42, 1)];
    app.update();

    let world = app.world_mut();
    let node = world
        .query_filtered::<&Node, With<OverlayMailComposeParcel>>()
        .single(world)
        .expect("source parcel root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(326.0), Val::Px(0.0), Val::Px(236.0), Val::Px(384.0)),
    );
    assert!(world.query::<&ImageNode>().iter(world).any(|image| {
        image.image.path().is_some_and(|path| path.to_string() == "original-ui/Title/674.png")
    }));
    let mut buttons = world.query::<(&OverlayButton, &Node)>();
    let (_, send) = buttons
        .iter(world)
        .find(|(button, _)| matches!(button, OverlayButton::SubmitMail))
        .expect("source send");
    assert_eq!(
        (send.left, send.top, send.width, send.height),
        (Val::Px(30.0), Val::Px(350.0), Val::Px(76.0), Val::Px(25.0)),
    );
    let (_, cell) = buttons
        .iter(world)
        .find(|(button, _)| matches!(button, OverlayButton::MailParcelSlot(0)))
        .expect("first source parcel cell");
    assert_eq!(
        (cell.left, cell.top, cell.width, cell.height),
        (Val::Px(27.0), Val::Px(311.0), Val::Px(35.0), Val::Px(31.0)),
    );
    let bag = world
        .query_filtered::<&Node, With<OverlayInventory>>()
        .single(world)
        .expect("ordinary bag root");
    assert_eq!((bag.left, bag.top), (Val::Px(0.0), Val::Px(0.0)));
}

#[test]
fn open_parcel_uses_recipient_prompt_without_overwriting_a_saved_draft() {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<MailComposeUi>()
        .init_resource::<InventoryModel>()
        .init_resource::<mail_parcel::MailParcelUi>()
        .init_resource::<MailServiceInbox>()
        .init_resource::<PendingOperations>()
        .add_systems(Update, process_mail_service_inbox);
    announce_mail_stream(&mut app);
    app.world_mut()
        .resource_mut::<MailServiceInbox>()
        .push(MailServiceEvent::OpenParcel);
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    let compose = app.world().resource::<MailComposeUi>();
    let prompt = compose.recipient_prompt.as_ref().expect("source recipient prompt");
    assert_eq!(prompt.target_kind, MailComposeKind::Parcel);
    assert!(state.mail_recipient_prompt_active);

    drop(compose);
    drop(state);
    {
        let mut compose = app.world_mut().resource_mut::<MailComposeUi>();
        compose.recipient_prompt = None;
        compose.parcel_draft = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Saved".into(),
            message: "do not retarget".into(),
            ..default()
        });
    }
    app.world_mut()
        .resource_mut::<MailServiceInbox>()
        .push(MailServiceEvent::OpenParcel);
    app.update();
    assert!(app.world().resource::<MailComposeUi>().recipient_prompt.is_none());
    assert_eq!(
        app.world().resource::<MailComposeUi>().last_notice.as_deref(),
        Some("Finish or cancel the current parcel first"),
    );
}

#[test]
fn quote_singleflight_ignores_stale_cost_and_requires_current_live_ids() {
    let mut ui = mail_parcel::MailParcelUi::default();
    let inventory = InventoryModel { items: vec![source_item(41, 0)], ..default() };
    let mut draft = mir2_ui_core::state::MailComposeDraft {
        gold: 100,
        attachment_unique_ids: vec![41],
        ..default()
    };
    assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
    draft.gold = 200;
    // This response arrived before the synchronizer has refreshed its cached
    // desired fingerprint; the live draft still rejects the old quote.
    ui.apply_cost(15, Some(&draft), &inventory);
    assert!(!ui.quote_is_current(&draft, &inventory));
    assert!(ui.begin_quote(&draft, &inventory, 3).is_some());
    ui.apply_cost(25, Some(&draft), &inventory);
    assert!(ui.quote_is_current(&draft, &inventory));

    let stale_inventory = InventoryModel::default();
    assert!(!ui.quote_is_current(&draft, &stale_inventory));
}

#[test]
fn quote_is_invalidated_by_live_attachment_pricing_changes() {
    let mut ui = mail_parcel::MailParcelUi::default();
    let mut item = source_item(41, 0);
    item.tooltip_source.as_mut().expect("concrete source").info.price = 1_000;
    let mut inventory = InventoryModel { items: vec![item], ..default() };
    let draft = mir2_ui_core::state::MailComposeDraft {
        attachment_unique_ids: vec![41],
        ..default()
    };

    assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
    ui.apply_cost(50, Some(&draft), &inventory);
    assert!(ui.quote_is_current(&draft, &inventory));

    // A V2 item refresh can change a stack's current quantity without
    // changing its UID. MailCost prices the whole live item, so that old
    // server result must no longer enable Send.
    inventory.items[0].quantity = 2;
    assert!(!ui.quote_is_current(&draft, &inventory));
    assert!(ui.begin_quote(&draft, &inventory, 2).is_some());
}

#[test]
fn parcel_lock_is_exact_and_local_cancel_recovers_if_echoes_are_dropped() {
    let inventory = InventoryModel {
        items: vec![source_item(41, 0), source_item(42, 1)],
        ..default()
    };
    let mut ui = mail_parcel::MailParcelUi::default();
    let mut draft = mir2_ui_core::state::MailComposeDraft::default();

    assert!(ui.attach(&mut draft, &inventory, 41));
    assert!(ui.blocks_item(41), "selection locks before the true echo");
    assert!(!ui.blocks_item(42));
    ui.apply_lock_receipt(41, true);
    ui.apply_lock_receipt(9_999, true);
    assert!(ui.blocks_item(41));
    assert!(!ui.blocks_item(9_999), "unknown echoed IDs cannot create a permanent lock");

    assert_eq!(ui.detach_at(&mut draft, 0), Some(41));
    assert!(!ui.blocks_item(41), "cancel is locally authoritative when false echo is lost");
    ui.apply_lock_receipt(41, true);
    ui.apply_lock_receipt(41, false);
    assert!(!ui.blocks_item(41), "late cancel echoes stay harmless after release");

    assert!(ui.attach(&mut draft, &inventory, 41));
    ui.apply_lock_receipt(41, false);
    assert!(ui.blocks_item(41), "a delayed old false cannot unlock a reselected item");
    ui.complete_send();
    assert!(!ui.blocks_item(41));
    draft.attachment_unique_ids.clear();

    assert!(ui.attach(&mut draft, &inventory, 41));
    assert!(ui.session_reset(1).is_none());
    assert!(ui.session_reset(2).is_some());
    ui.apply_lock_receipt(41, true);
    assert!(!ui.blocks_item(41), "reset rejects a previous session's true echo");
}

#[test]
fn parcel_locked_bag_item_is_dimmed_and_cannot_start_a_drag() {
    let inventory = InventoryModel { items: vec![source_item(41, 0)], ..default() };
    let mut state = NativePlayerUiState::default();
    let mut parcel = mail_parcel::MailParcelUi::default();
    let mut draft = mir2_ui_core::state::MailComposeDraft::default();
    assert!(parcel.attach(&mut draft, &inventory, 41));

    let cursor = Vec2::new(
        state.inventory_window.left + INVENTORY_GRID_ORIGIN.x as f32 + 2.0,
        state.inventory_window.top + INVENTORY_GRID_ORIGIN.y as f32 + 2.0,
    );
    assert!(
        inventory_item_drag_at_cursor(&state, &inventory, Some(&parcel), cursor).is_none(),
        "the precise selected UID cannot enter the bag drag pipeline"
    );
    assert!(
        inventory_item_drag_at_cursor(&state, &inventory, None, cursor).is_some(),
        "ordinary operation locking remains separate from mail selection"
    );

    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.mail_inventory_visible = true;
        state.core.mail_compose = Some(draft);
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;
    app.world_mut().resource_mut::<InventoryModel>().items = inventory.items;
    *app.world_mut().resource_mut::<mail_parcel::MailParcelUi>() = parcel;
    app.update();

    let dim_gray = Color::srgba(105.0 / 255.0, 105.0 / 255.0, 105.0 / 255.0, 0.8);
    assert!(
        app.world_mut()
            .query::<&ImageNode>()
            .iter(app.world())
            .any(|image| image.color == dim_gray),
        "a locked carried cell keeps Crystal's DimGray 0.8 presentation"
    );
}

#[test]
fn parcel_selected_uid_blocks_the_overlay_action_chain() {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<MailModel>()
        .init_resource::<ShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<crate::social::SocialModel>();
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..default()
    });
    super::tests::init_overlay_button_test_resources(&mut app);
    app.add_systems(Update, process_overlay_buttons);
    app.world_mut().resource_mut::<InventoryModel>().items = vec![source_item(41, 0)];
    {
        let inventory = app.world().resource::<InventoryModel>().clone();
        let mut parcel = app.world_mut().resource_mut::<mail_parcel::MailParcelUi>();
        assert!(parcel.attach(
            &mut mir2_ui_core::state::MailComposeDraft::default(),
            &inventory,
            41,
        ));
    }

    fn press(app: &mut App, button: OverlayButton) {
        let entity = app.world_mut().spawn((Interaction::Pressed, button, Button)).id();
        app.update();
        app.world_mut().despawn(entity);
    }

    press(&mut app, OverlayButton::InspectBag(0));
    assert!(app.world().resource::<NativePlayerUiState>().inspect.is_none());

    {
        let item = app.world().resource::<InventoryModel>().items[0].clone();
        app.world_mut().resource_mut::<NativePlayerUiState>().inspect = Some(inspect_from_item(&item));
    }
    press(&mut app, OverlayButton::UseInspected);
    press(&mut app, OverlayButton::EquipInspected);
    press(&mut app, OverlayButton::DropInspected);
    press(&mut app, OverlayButton::SplitInspected);
    press(&mut app, OverlayButton::ArmMoveInspected);
    press(&mut app, OverlayButton::ArmMergeInspected);
    {
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.drop_confirmation.is_none());
        assert!(state.inventory_operation.is_none());
    }
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    app.world_mut().resource_mut::<NativePlayerUiState>().inventory_delete_mode = true;
    press(&mut app, OverlayButton::InspectBag(0));
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .inventory_delete_prompt
        .is_none());

    app.world_mut().resource_mut::<NativePlayerUiState>().inventory_operation = Some(
        InventoryOperationDraft::Move {
            source_slot: 1,
            unique_id: 42,
        },
    );
    press(&mut app, OverlayButton::InspectBag(0));
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty(), "a non-mail source cannot swap into a locked attachment cell");
}

#[test]
fn production_parcel_has_no_quote_without_trusted_stream_and_same_marker_keeps_entered_slot() {
    let mut app=App::new(); app.add_plugins(Mir2NativeMailParcelServicePlugin);
    app.world_mut().resource_mut::<NativeShellModel>().screen=NativeShellScreen::InGame;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel=mir2_ui_core::state::UiPanel::Mail;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{gold:700,..default()});
    app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;
    app.update(); assert!(drain_entered_mail_fixture(&mut app).is_empty());
    assert_eq!(native_mail_parcel_quote_state(app.world()).unwrap().stream_epoch,None);
    announce_mail_stream(&mut app); app.update();
    assert_eq!(drain_entered_mail_fixture(&mut app).len(),1);
    announce_mail_stream(&mut app); app.update();
    assert!(drain_entered_mail_fixture(&mut app).is_empty());
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_delivery(crate::mail_service::MailServiceDelivery{epoch:TEST_MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
    app.update();
    let quote=native_mail_parcel_quote_state(app.world()).unwrap();
    assert!(quote.current); assert_eq!(quote.postage,Some(70)); assert!(!quote.pending);
}

#[test]
fn native_new_stream_resets_tombstone_once_and_scheduler_observes_before_reservation() {
    let mut app=App::new();
    app.init_resource::<NativePlayerUiState>().init_resource::<MailComposeUi>()
        .init_resource::<InventoryModel>().init_resource::<mail_parcel::MailParcelUi>()
        .init_resource::<PendingOperations>().init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<MailServiceInbox>().init_resource::<SessionResetRevision>()
        .insert_resource(NativeShellModel{screen:NativeShellScreen::InGame,..default()})
        .add_systems(Update,sync_mail_parcel_quote);
    announce_mail_stream(&mut app);
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel=mir2_ui_core::state::UiPanel::Mail;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{gold:700,..default()});
    app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;
    app.update(); assert_eq!(drain_entered_mail_fixture(&mut app).len(),1);
    app.world_mut().resource_mut::<SessionResetRevision>().0=1; app.update(); app.update();
    assert!(drain_entered_mail_fixture(&mut app).is_empty());
    let epoch=MailServiceStreamEpoch{run:u64::MAX,connection:u64::MAX};
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().start_stream(crate::mail_service::MailServiceStreamStarted{epoch}));
    app.update(); assert_eq!(drain_entered_mail_fixture(&mut app).len(),1,"true new stream discards old tombstone before reserving");
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().start_stream(crate::mail_service::MailServiceStreamStarted{epoch}));
    app.update(); assert!(drain_entered_mail_fixture(&mut app).is_empty(),"same marker cannot clear entered quote");
    assert!(!app.world_mut().resource_mut::<MailServiceInbox>().push_delivery(crate::mail_service::MailServiceDelivery{epoch:TEST_MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
    app.add_systems(Update,process_mail_service_inbox.before(sync_mail_parcel_quote));
    assert!(app.world_mut().resource_mut::<MailServiceInbox>().push_delivery(crate::mail_service::MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:140}}));
    app.update(); let quote=native_mail_parcel_quote_state(app.world()).unwrap();
    assert!(quote.current); assert_eq!(quote.postage,Some(140));
}
