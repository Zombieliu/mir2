use super::*;

fn feedback_test_app() -> App {
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
