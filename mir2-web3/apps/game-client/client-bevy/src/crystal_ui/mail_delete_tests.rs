use super::*;

fn delete_test_app() -> App {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
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
        (process_overlay_keyboard, process_overlay_buttons).chain(),
    );
    app
}

fn mail(id: u64, gold: u32, item: Option<&str>, locked: bool) -> crate::mail::MailMessage {
    crate::mail::MailMessage {
        id,
        sender: "System".into(),
        subject: format!("Mail {id}"),
        gold,
        items: item
            .into_iter()
            .map(|name| MailAttachment {
                unique_id: Some(id + 1000),
                item_index: Some(7),
                name: Some(name.into()),
                count: 2,
                current_dura: 4,
                max_dura: 8,
                ..Default::default()
            })
            .collect(),
        locked,
        ..Default::default()
    }
}

fn press_button(app: &mut App, button: OverlayButton) {
    let entity = app
        .world_mut()
        .spawn((Interaction::Pressed, button, Button))
        .id();
    app.update();
    app.world_mut().despawn(entity);
}

#[test]
fn attachment_delete_opens_a_confirm_but_plain_mail_stays_direct_and_locked_mail_rejects() {
    let mut app = delete_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
    }
    app.world_mut()
        .resource_mut::<MailModel>()
        .mails
        .extend([
            mail(10, 0, None, false),
            mail(11, 0, Some("Potion"), false),
            mail(12, 1, None, true),
        ]);

    press_button(&mut app, OverlayButton::DeleteMail(11));
    let state = app.world().resource::<NativePlayerUiState>();
    assert_eq!(
        state.mail_delete_prompt.as_ref().map(|prompt| prompt.mail_id),
        Some(11),
        "items or gold use MailDialog's source warning"
    );
    assert!(state.blocks_gameplay_keys());
    assert!(state.blocks_world_click());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_delete_prompt = None;
    press_button(&mut app, OverlayButton::DeleteMail(10));
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::DeleteMail { mail_id: 10 }],
        "a no-attachment row remains MailDialog's direct-delete path"
    );

    press_button(&mut app, OverlayButton::DeleteMail(12));
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_delete_prompt
        .is_none());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn attachment_confirm_rechecks_identity_contents_and_lock_before_queueing() {
    let mut state = NativePlayerUiState::default();
    let mut mail_model = MailModel {
        mails: vec![mail(7, 0, Some("Potion"), false), mail(8, 25, None, false)],
        selected_id: Some(7),
    };
    state.mail_delete_prompt = mail_delete_prompt_for_message(&mail_model.mails[0]);
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();

    mail_model.mails.swap(0, 1);
    assert!(mail_delete_prompt_is_current(
        state.mail_delete_prompt.as_ref().expect("captured prompt"),
        &mail_model,
    ));
    assert!(confirm_mail_delete(
        &mut state,
        &mut mail_model,
        &mut intents,
        &mut pending,
    ));
    assert_eq!(mail_model.selected_id, None);
    assert_eq!(
        intents.drain_intents(),
        vec![NativePlayerUiIntent::DeleteMail { mail_id: 7 }],
        "reordering must retain the captured MailID rather than a row index"
    );

    for altered in [
        mail(7, 1, Some("Potion"), false),
        mail(7, 0, Some("Elixir"), false),
        mail(7, 0, Some("Potion"), true),
    ] {
        let mut state = NativePlayerUiState::default();
        let mut mail_model = MailModel {
            mails: vec![mail(7, 0, Some("Potion"), false)],
            selected_id: Some(7),
        };
        state.mail_delete_prompt = mail_delete_prompt_for_message(&mail_model.mails[0]);
        mail_model.mails[0] = altered;
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        assert!(!confirm_mail_delete(
            &mut state,
            &mut mail_model,
            &mut intents,
            &mut pending,
        ));
        assert!(state.mail_delete_prompt.is_none());
        assert!(intents.drain_intents().is_empty());
    }

    let mut state = NativePlayerUiState::default();
    let mut mail_model = MailModel {
        mails: vec![mail(7, 0, Some("Potion"), false)],
        selected_id: Some(7),
    };
    state.mail_delete_prompt = mail_delete_prompt_for_message(&mail_model.mails[0]);
    mail_model.mails.clear();
    assert!(!confirm_mail_delete(
        &mut state,
        &mut mail_model,
        &mut NativePlayerUiIntentQueue::default(),
        &mut PendingOperations::default(),
    ));
    assert!(state.mail_delete_prompt.is_none(), "a deleted row cannot be retargeted");
}

#[test]
fn yes_and_enter_consume_the_close_frame_and_pending_deduplicates_the_same_mail() {
    let mut app = delete_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.extend([
        mail(20, 0, Some("Potion"), false),
        mail(21, 0, None, false),
    ]);
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_delete_prompt =
        mail_delete_prompt_for_message(&app.world().resource::<MailModel>().mails[0]);

    let yes = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::MailDeleteConfirm, Button))
        .id();
    let covered_delete = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::DeleteMail(21), Button))
        .id();
    app.update();
    app.world_mut().despawn(yes);
    app.world_mut().despawn(covered_delete);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_delete_input_consumed);
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::DeleteMail { mail_id: 20 }],
        "the covered button cannot also fire in a Yes frame"
    );

    let prompt = mail_delete_prompt_for_message(&app.world().resource::<MailModel>().mails[0]);
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.mail_delete_prompt = prompt;
        state.mail_delete_input_consumed = false;
    }
    press_button(&mut app, OverlayButton::MailDeleteConfirm);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_delete_prompt
        .is_some(), "a duplicate pending delete remains retryable after a receipt");
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    let mut keyboard_app = delete_test_app();
    keyboard_app
        .world_mut()
        .resource_mut::<MailModel>()
        .mails
        .extend([mail(30, 0, Some("Potion"), false), mail(31, 0, None, false)]);
    keyboard_app
        .world_mut()
        .resource_mut::<NativePlayerUiState>()
        .mail_delete_prompt = mail_delete_prompt_for_message(
        &keyboard_app.world().resource::<MailModel>().mails[0],
    );
    let covered_delete = keyboard_app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::DeleteMail(31), Button))
        .id();
    keyboard_app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    keyboard_app.update();
    keyboard_app.world_mut().despawn(covered_delete);
    assert_eq!(
        keyboard_app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::DeleteMail { mail_id: 30 }],
        "Enter cannot leak to a covered mail action after confirmation closes"
    );
}

#[test]
fn no_escape_close_and_session_reset_clear_the_prompt_without_sending() {
    let mut app = delete_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.push(mail(40, 0, Some("Potion"), false));
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_delete_prompt =
        mail_delete_prompt_for_message(&app.world().resource::<MailModel>().mails[0]);
    press_button(&mut app, OverlayButton::MailDeleteCancel);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_delete_prompt
        .is_none());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_delete_prompt =
        mail_delete_prompt_for_message(&app.world().resource::<MailModel>().mails[0]);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_delete_prompt
        .is_none());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.mail_delete_prompt = Some(MailDeletePrompt {
        mail_id: 40,
        gold: 0,
        items: vec![],
    });
    state.close_windows();
    assert!(state.mail_delete_prompt.is_none());
    state.mail_delete_prompt = Some(MailDeletePrompt {
        mail_id: 40,
        gold: 0,
        items: vec![],
    });
    state.reset_session();
    assert!(state.mail_delete_prompt.is_none());
}

#[test]
fn rendered_warning_uses_a_modal_source_frame_and_blocks_pointer_input() {
    let mut app = super::tests::overlay_render_test_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_delete_prompt = Some(
        MailDeletePrompt {
            mail_id: 50,
            gold: 1,
            items: vec![],
        },
    );
    app.update();
    let world = app.world_mut();
    let (node, focus) = world
        .query_filtered::<(&Node, &FocusPolicy), With<OverlayMailDeleteModal>>()
        .single(world)
        .expect("mail deletion modal root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(*focus, FocusPolicy::Block);
    assert_eq!(node.width, Val::Px(1024.0));
    assert_eq!(node.height, Val::Px(768.0));
}
