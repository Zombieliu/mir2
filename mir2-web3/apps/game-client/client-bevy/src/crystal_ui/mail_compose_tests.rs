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
fn mail_body_utf16_budget_matches_with_and_without_native_editor() {
    for native_editor in [true, false] {
        for kind in [MailComposeKind::Letter, MailComposeKind::Parcel] {
            for (input, expected) in [
                ("a".repeat(501), "a".repeat(500)),
                ("😀".repeat(251), "😀".repeat(250)),
                (format!("{}😀", "中".repeat(499)), "中".repeat(499)),
            ] {
                let mut app = input_app();
                if !native_editor {
                    app.world_mut().remove_resource::<mail_editor::MailLetterEditor>();
                }
                app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose =
                    Some(mir2_ui_core::state::MailComposeDraft {
                        recipient: "Receiver".into(), ..default()
                    });
                {
                    let mut compose = app.world_mut().resource_mut::<MailComposeUi>();
                    compose.kind = kind;
                    compose.focus = MailComposeFocus::Message;
                }
                type_text(&mut app, KeyCode::KeyA, &input);
                let message = &app.world().resource::<NativePlayerUiState>()
                    .core.mail_compose.as_ref().unwrap().message;
                assert_eq!(message, &expected, "native editor={native_editor}");
                type_text(&mut app, KeyCode::Enter, "");
                let message = &app.world().resource::<NativePlayerUiState>()
                    .core.mail_compose.as_ref().unwrap().message;
                let expected = if expected.encode_utf16().count() < 500 {
                    format!("{expected}\n")
                } else { expected };
                assert_eq!(message, &expected);
            }
        }
    }
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
            stamped: false,
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
            stamped: false,
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

fn mail_wheel_layout(text: &str) -> Vec<friend_dialog::text_editor::VisualLine> {
    let mut starts = text
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    starts.insert(0, 0);
    starts
        .into_iter()
        .enumerate()
        .map(|(index, start)| {
            let end = text[start..]
                .find('\n')
                .map(|offset| start + offset)
                .unwrap_or(text.len());
            friend_dialog::text_editor::VisualLine {
                y: index as f32 * 20.0,
                height: 20.0,
                stops: vec![
                    friend_dialog::text_editor::CaretStop { byte: start, x: 0.0 },
                    friend_dialog::text_editor::CaretStop { byte: end, x: 20.0 },
                ],
            }
        })
        .collect()
}

fn mail_wheel_app(kind: MailComposeKind) -> (App, Entity) {
    let body = "x\n".repeat(20);
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<mail_compose_drag::MailLetterWindow>()
        .init_resource::<mail_parcel::MailParcelUi>()
        .init_resource::<mail_editor::MailLetterEditor>()
        .init_resource::<PendingOperations>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .add_message::<MouseWheel>()
        .add_systems(Update, process_mail_letter_editor_wheel);
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: body.clone(),
            ..default()
        });
    }
    app.world_mut().resource_mut::<MailComposeUi>().kind = kind;
    {
        let mut editor = app.world_mut().resource_mut::<mail_editor::MailLetterEditor>();
        editor.sync(true, Some(&body));
        editor.install_layout(mail_wheel_layout(&body));
    }
    let (origin, body_rect) = if kind == MailComposeKind::Parcel {
        (
            app.world().resource::<mail_parcel::MailParcelUi>().window.position,
            mail_parcel::MAIL_PARCEL_BODY_RECT,
        )
    } else {
        (
            app.world().resource::<mail_compose_drag::MailLetterWindow>().position,
            mail_editor::MAIL_LETTER_BODY_RECT,
        )
    };
    let logical = origin
        + Vec2::new(body_rect.left, body_rect.top)
        + mail_editor::MAIL_LETTER_CONTENT_INSET
        + Vec2::new(10.0, 10.0);
    let mut window = Window::default();
    window.focused = true;
    window.resolution.set(2048.0, 1536.0);
    window.set_cursor_position(Some(logical * 2.0));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    (app, window)
}

fn wheel(app: &mut App, window: Entity, y: f32, unit: MouseScrollUnit) {
    app.world_mut().write_message(MouseWheel {
        phase: bevy::input::touch::TouchPhase::Moved,
        unit,
        x: 0.0,
        y,
        window,
    });
    app.update();
}

#[test]
fn scaled_mail_body_wheel_routes_letter_and_parcel_without_leaking_past_guards() {
    for kind in [MailComposeKind::Letter, MailComposeKind::Parcel] {
        let (mut app, window) = mail_wheel_app(kind);
        let before = app
            .world()
            .resource::<mail_editor::MailLetterEditor>()
            .active_editor()
            .expect("mail editor")
            .selection();
        wheel(&mut app, window, -400.0, MouseScrollUnit::Pixel);
        assert_eq!(
            app.world().resource::<mail_editor::MailLetterEditor>().scroll().y,
            200.0,
            "{kind:?} converts physical pixel wheel delta through the 2x stage scale"
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .menu_pointer_consumed = false;
        wheel(&mut app, window, -100.0, MouseScrollUnit::Line);
        assert_eq!(
            app.world().resource::<mail_editor::MailLetterEditor>().scroll().y,
            259.0,
            "{kind:?} consumes a scaled in-viewport line wheel"
        );
        assert_eq!(
            app.world()
                .resource::<mail_editor::MailLetterEditor>()
                .active_editor()
                .expect("mail editor")
                .selection(),
            before,
            "wheel does not change selection"
        );

        // A new input frame clears this pointer latch in the normal host before
        // independent pointer consumers run.
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .menu_pointer_consumed = false;
        app.world_mut()
            .get_mut::<Window>(window)
            .expect("window")
            .set_cursor_position(Some(Vec2::new(2.0, 2.0)));
        let before_outside = app.world().resource::<mail_editor::MailLetterEditor>().scroll();
        wheel(&mut app, window, 1.0, MouseScrollUnit::Line);
        assert_eq!(
            app.world().resource::<mail_editor::MailLetterEditor>().scroll(),
            before_outside,
            "wheel outside the body is not routed to mail"
        );

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .mail_feedback_prompt = Some("covered".into());
        let before_covered = app.world().resource::<mail_editor::MailLetterEditor>().scroll();
        wheel(&mut app, window, -1.0, MouseScrollUnit::Line);
        assert_eq!(
            app.world().resource::<mail_editor::MailLetterEditor>().scroll(),
            before_covered,
            "covered compose ignores queued wheel input"
        );
    }
}

#[test]
fn mail_body_wheel_ignores_pending_send_and_focus_loss() {
    let (mut pending_app, pending_window) = mail_wheel_app(MailComposeKind::Letter);
    assert!(pending_app.world_mut().resource_mut::<PendingOperations>().try_begin(
        crate::pending_operations::PendingOperationKey::SendMail {
            recipient: "Receiver".into(),
            message: "body".into(),
            gold: 0,
            attachment_unique_ids: vec![],
        },
    ));
    wheel(&mut pending_app, pending_window, -1.0, MouseScrollUnit::Line);
    assert_eq!(
        pending_app
            .world()
            .resource::<mail_editor::MailLetterEditor>()
            .scroll()
            .y,
        0.0,
        "pending send freezes mail body input"
    );

    let (mut unfocused_app, unfocused_window) = mail_wheel_app(MailComposeKind::Letter);
    unfocused_app
        .world_mut()
        .get_mut::<Window>(unfocused_window)
        .expect("window")
        .focused = false;
    wheel(&mut unfocused_app, unfocused_window, -1.0, MouseScrollUnit::Line);
    assert_eq!(
        unfocused_app
            .world()
            .resource::<mail_editor::MailLetterEditor>()
            .scroll()
            .y,
        0.0,
        "focus loss discards mail wheel input"
    );
}
