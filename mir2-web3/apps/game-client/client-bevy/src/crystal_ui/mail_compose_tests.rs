use super::*;

fn input_app() -> App {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<mail_editor::MailLetterEditor>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<MailModel>()
        .init_resource::<ShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<crate::social::SocialModel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::audio::NativeUiAudioQueue>()
        .add_message::<KeyboardInput>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
    super::tests::init_overlay_button_test_resources(&mut app);
    app.add_systems(
        Update,
        (
            process_queued_mail_letter_request,
            process_overlay_keyboard,
            process_overlay_buttons,
        )
            .chain(),
    );
    app
}

fn press(app: &mut App, button: OverlayButton) {
    let entity = app.world_mut().spawn((Interaction::Pressed, button, Button)).id();
    app.update();
    app.world_mut().despawn(entity);
}

fn type_text(app: &mut App, key_code: KeyCode, text: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key: bevy::input::keyboard::Key::Character(text.into()),
        state: ButtonState::Pressed,
        text: Some(text.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

#[test]
fn letter_renderer_uses_source_root_full_multiline_text_and_visible_focus() {
    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "first line\nsecond line".into(),
            ..default()
        });
    }
    {
        let mut compose = app.world_mut().resource_mut::<MailComposeUi>();
        compose.kind = MailComposeKind::Letter;
        compose.focus = MailComposeFocus::Message;
    }
    app.update();
    let world = app.world_mut();
    let node = world
        .query_filtered::<&Node, With<OverlayMailComposeLetter>>()
        .single(world)
        .expect("letter compose root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!((node.left, node.top, node.width, node.height), (
        Val::Px(100.0), Val::Px(100.0), Val::Px(236.0), Val::Px(300.0),
    ));
    assert!(world.query::<&Text>().iter(world).any(|text| text.0 == "first line\nsecond line"));
    assert!(world.query::<&ImageNode>().iter(world).any(|image| {
        image.image.path().is_some_and(|path| path.to_string() == "original-ui/Title/671.png")
    }), "the source Letter frame must be present");
    let mut buttons = world.query::<(&OverlayButton, &Node)>();
    let (_, send) = buttons
        .iter(world)
        .find(|(button, _)| matches!(button, OverlayButton::SubmitMail))
        .expect("source send control");
    assert_eq!(
        (send.left, send.top, send.width, send.height),
        (Val::Px(30.0), Val::Px(265.0), Val::Px(76.0), Val::Px(25.0)),
    );
    let (_, cancel) = buttons
        .iter(world)
        .find(|(button, node)| matches!(button, OverlayButton::CancelMailCompose) && node.top == Val::Px(265.0))
        .expect("source cancel control");
    assert_eq!(
        (cancel.left, cancel.top, cancel.width, cancel.height),
        (Val::Px(135.0), Val::Px(265.0), Val::Px(68.0), Val::Px(25.0)),
    );
    let mut fields = world.query::<(&Node, &BorderColor)>();
    let (_, border) = fields
        .iter(world)
        .find(|(field, _)| {
            (field.left, field.top, field.width, field.height)
                == (Val::Px(15.0), Val::Px(92.0), Val::Px(202.0), Val::Px(165.0))
        })
        .expect("focused full-body field");
    assert_eq!(*border, BorderColor::all(Color::srgb(0.0, 1.0, 0.0)));
    let tag = world
        .query::<&mail_editor::MailLetterEditText>()
        .single(world)
        .expect("shaped message text tag");
    assert_eq!(tag.viewport, [198.0, 161.0]);
}

#[test]
fn letter_renderer_follows_the_movable_source_window_position() {
    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "body".into(),
            ..default()
        });
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Letter;
    app.world_mut()
        .resource_mut::<mail_compose_drag::MailLetterWindow>()
        .position = Vec2::new(420.0, 215.0);
    app.update();
    let world = app.world_mut();
    let node = world
        .query_filtered::<&Node, With<OverlayMailComposeLetter>>()
        .single(world)
        .expect("movable letter root");
    assert_eq!((node.left, node.top), (Val::Px(420.0), Val::Px(215.0)));
}

#[test]
fn write_recipient_prompt_confirms_exact_recipient_and_consumes_covered_parcel_action() {
    let mut app = input_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
    }
    press(&mut app, OverlayButton::OpenMailCompose);
    assert!(app.world().resource::<MailComposeUi>().recipient_prompt.is_some());
    assert!(app.world().resource::<NativePlayerUiState>().mail_recipient_prompt_active);
    type_text(&mut app, KeyCode::KeyR, "Receiver");
    assert_eq!(
        app.world().resource::<MailComposeUi>().recipient_prompt.as_ref().map(|prompt| prompt.recipient.as_str()),
        Some("Receiver"),
        "the visible Prguse660 field receives the keyboard text",
    );
    let submit = app.world_mut().spawn((Interaction::Pressed, OverlayButton::MailRecipientSubmit, Button)).id();
    let covered = app.world_mut().spawn((Interaction::Pressed, OverlayButton::OpenMailParcelCompose, Button)).id();
    app.update();
    app.world_mut().despawn(submit);
    app.world_mut().despawn(covered);
    {
        let state = app.world().resource::<NativePlayerUiState>();
        let compose = app.world().resource::<MailComposeUi>();
        assert_eq!(state.core.mail_compose.as_ref().map(|draft| draft.recipient.as_str()), Some("Receiver"));
        assert_eq!(compose.kind, MailComposeKind::Letter);
        assert_eq!(compose.focus, MailComposeFocus::Message);
        assert!(compose.recipient_prompt.is_none());
        assert!(state.mail_recipient_input_consumed);
    }

    // The following physical frame edits the source Letter body directly;
    // it never cycles back into a hidden recipient or gold adapter field.
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_recipient_input_consumed = false;
    type_text(&mut app, KeyCode::KeyM, "Line one");
    type_text(&mut app, KeyCode::Enter, "");
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().map(|draft| draft.message.as_str()),
        Some("Line one\n"),
    );
}

#[test]
fn recipient_cancel_and_letter_send_keep_input_isolated_and_draft_authoritative() {
    let mut app = input_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
    }
    press(&mut app, OverlayButton::OpenMailCompose);
    press(&mut app, OverlayButton::MailRecipientCancel);
    assert!(app.world().resource::<MailComposeUi>().recipient_prompt.is_none());
    assert!(app.world().resource::<NativePlayerUiState>().mail_recipient_input_consumed);

    // A later physical input frame permits another source Write action.
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_recipient_input_consumed = false;
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "first\nsecond".into(),
            ..default()
        });
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Letter;
    press(&mut app, OverlayButton::SubmitMail);
    assert_eq!(app.world().resource::<MailComposeUi>().last_notice.as_deref(), Some("Sending mail…"));
    assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_some(), "only a successful receipt closes the draft");
    assert_eq!(
        app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents(),
        vec![NativePlayerUiIntent::SendMail {
            recipient: "Receiver".into(),
            message: "first\nsecond".into(),
            gold: 0,
            attachment_unique_ids: vec![],
        }],
    );
}

#[test]
fn parcel_to_letter_or_reply_never_sends_hidden_gold_or_items_and_keeps_parcel_draft() {
    let mut app = input_app();
    let parcel = mir2_ui_core::state::MailComposeDraft {
        recipient: "Parcel recipient".into(),
        message: "Parcel message".into(),
        gold: 700,
        attachment_unique_ids: vec![71],
    };
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.mail_compose = Some(parcel.clone());
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;

    press(&mut app, OverlayButton::OpenMailCompose);
    {
        let mut compose = app.world_mut().resource_mut::<MailComposeUi>();
        compose.recipient_prompt.as_mut().expect("letter recipient prompt").recipient = "Letter recipient".into();
    }
    press(&mut app, OverlayButton::MailRecipientSubmit);
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_recipient_input_consumed = false;
    {
        let state = app.world().resource::<NativePlayerUiState>();
        let compose = app.world().resource::<MailComposeUi>();
        let letter = state.core.mail_compose.as_ref().expect("letter draft");
        assert_eq!(compose.kind, MailComposeKind::Letter);
        assert_eq!(letter.gold, 0);
        assert!(letter.attachment_unique_ids.is_empty());
        assert_eq!(compose.parcel_draft.as_ref(), Some(&parcel));
    }
    app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose.as_mut().expect("letter").message = "Letter message".into();
    press(&mut app, OverlayButton::SubmitMail);
    assert_eq!(
        app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents(),
        vec![NativePlayerUiIntent::SendMail {
            recipient: "Letter recipient".into(),
            message: "Letter message".into(),
            gold: 0,
            attachment_unique_ids: vec![],
        }],
    );

    // A non-correlated Send receipt can close only the active surface, so do
    // not let a Reply replace the submitted Letter while that request waits.
    app.world_mut().resource_mut::<MailModel>().mails.push(MailMessage {
        id: 99,
        sender: "Reply sender".into(),
        can_reply: true,
        ..default()
    });
    press(&mut app, OverlayButton::MailReply(99));
    assert_eq!(app.world().resource::<MailComposeUi>().kind, MailComposeKind::Letter);
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().map(|draft| draft.recipient.as_str()),
        Some("Letter recipient"),
        "Reply is blocked until the uncorrelated Send result resolves",
    );
}

#[test]
fn submitted_letter_body_is_frozen_until_the_uncorrelated_send_receipt() {
    let mut app = input_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "submitted body".into(),
            ..default()
        });
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Letter;
    press(&mut app, OverlayButton::SubmitMail);
    type_text(&mut app, KeyCode::KeyX, " later edit");
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().map(|draft| draft.message.as_str()),
        Some("submitted body"),
    );
    assert_eq!(app.world().resource::<MailComposeUi>().last_notice.as_deref(), Some("Sending mail…"));
}

#[test]
fn queued_friend_inspect_and_bond_letter_request_uses_the_safe_letter_transition() {
    let mut app = input_app();
    let parcel = mir2_ui_core::state::MailComposeDraft {
        recipient: "Parcel recipient".into(),
        message: "Parcel message".into(),
        gold: 500,
        attachment_unique_ids: vec![42],
    };
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.mail_compose = Some(parcel.clone());
        state.request_mail_letter("External recipient".into());
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    let compose = app.world().resource::<MailComposeUi>();
    let letter = state.core.mail_compose.as_ref().expect("safe letter draft");
    assert_eq!(letter.recipient, "External recipient");
    assert_eq!(letter.gold, 0);
    assert!(letter.attachment_unique_ids.is_empty());
    assert_eq!(compose.parcel_draft.as_ref(), Some(&parcel));
    assert!(state.queued_mail_letter_recipient.is_none());
}
