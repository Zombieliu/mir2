use super::*;

fn target_state() -> (
    NativeShellModel,
    NativePlayerUiState,
    MailComposeUi,
    mail_editor::MailLetterEditor,
    PendingOperations,
) {
    let shell = NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..default()
    };
    let mut state = NativePlayerUiState::default();
    state.core.panel = mir2_ui_core::state::UiPanel::Mail;
    state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
        recipient: "Receiver".into(),
        message: "Body".into(),
        ..default()
    });
    let mut compose = MailComposeUi {
        focus: MailComposeFocus::Message,
        kind: MailComposeKind::Letter,
        ..default()
    };
    compose.advance_draft_epoch();
    let mut editor = mail_editor::MailLetterEditor::default();
    editor.sync(true, Some("Body"));
    (shell, state, compose, editor, PendingOperations::default())
}

#[test]
fn target_binds_kind_recipient_draft_epoch_editor_and_session() {
    let (shell, mut state, mut compose, editor, pending) = target_state();
    let before = active_mail_text_target(&shell, &state, &compose, &editor, &pending, 4)
        .expect("visible letter body");
    state.core.mail_compose.as_mut().unwrap().recipient = "Other".into();
    assert_ne!(
        before,
        active_mail_text_target(&shell, &state, &compose, &editor, &pending, 4)
            .expect("different recipient remains a live body")
    );
    compose.kind = MailComposeKind::Parcel;
    assert_ne!(
        before,
        active_mail_text_target(&shell, &state, &compose, &editor, &pending, 4)
            .expect("parcel remains a live body")
    );
    compose.advance_draft_epoch();
    assert_ne!(
        before,
        active_mail_text_target(&shell, &state, &compose, &editor, &pending, 5)
            .expect("new draft/session has a target")
    );
    state.core.mail_compose.as_mut().unwrap().message = "stale editor must not write".into();
    assert!(
        active_mail_text_target(&shell, &state, &compose, &editor, &pending, 5).is_none(),
        "a pending input read cannot overwrite a newly restored draft before editor sync"
    );
}

#[test]
fn target_cancels_while_mail_body_is_covered_or_send_is_pending() {
    let (shell, mut state, compose, editor, mut pending) = target_state();
    state.mail_feedback_prompt = Some("blocked".into());
    assert!(active_mail_text_target(&shell, &state, &compose, &editor, &pending, 0).is_none());
    state.mail_feedback_prompt = None;
    state.mail_recipient_prompt_active = true;
    assert!(active_mail_text_target(&shell, &state, &compose, &editor, &pending, 0).is_none());
    state.mail_recipient_prompt_active = false;
    assert!(pending.try_begin(crate::pending_operations::PendingOperationKey::SendMail {
        recipient: "Receiver".into(),
        message: "Body".into(),
        gold: 0,
        attachment_unique_ids: vec![],
    }));
    assert!(active_mail_text_target(&shell, &state, &compose, &editor, &pending, 0).is_none());
}
