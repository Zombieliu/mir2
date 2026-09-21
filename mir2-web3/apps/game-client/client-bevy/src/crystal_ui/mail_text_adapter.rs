//! Mail compose text ownership shared by native IME and the Windows clipboard.
//! This deliberately adapts the real Letter/Parcel editor instead of routing
//! mail through FriendDialog's unrelated modal and draft model.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailTextTarget {
    pub kind: MailComposeKind,
    pub recipient: String,
    pub draft_epoch: u64,
    pub editor_revision: u64,
    pub session_revision: u64,
}

fn interactive(
    state: &NativePlayerUiState,
    compose: &MailComposeUi,
    pending: &PendingOperations,
) -> bool {
    state.mail_open()
        && state.core.mail_compose.is_some()
        && matches!(compose.kind, MailComposeKind::Letter | MailComposeKind::Parcel)
        && (compose.kind == MailComposeKind::Letter
            || compose.focus == MailComposeFocus::Message)
        && !state.amount_modal_open()
        && state.mail_feedback_prompt.is_none()
        && !state.mail_feedback_input_consumed
        && !state.mail_recipient_prompt_active
        && !state.mail_recipient_input_consumed
        && state.mail_reader.is_none()
        && !state.mail_reader_input_consumed
        && state.mail_delete_prompt.is_none()
        && !state.mail_delete_input_consumed
        && state.storage_password_prompt.is_none()
        && state.storage_rental_confirmation.is_none()
        && !mail_compose_drag::covered(state)
        && !pending.has_pending_mail_send()
}

/// Returns the exact live Letter/Parcel draft identity.  It is intentionally
/// not a body-text comparison: equal content from a different recipient,
/// compose kind, draft instance, or session must reject delayed input.
pub fn active_mail_text_target(
    shell: &NativeShellModel,
    state: &NativePlayerUiState,
    compose: &MailComposeUi,
    editor: &mail_editor::MailLetterEditor,
    pending: &PendingOperations,
    session_revision: u64,
) -> Option<MailTextTarget> {
    if shell.screen != NativeShellScreen::InGame
        || !interactive(state, compose, pending)
        || !editor.focused()
    {
        return None;
    }
    let draft = state.core.mail_compose.as_ref()?;
    let active_editor = editor.active_editor()?;
    if active_editor.text() != draft.message {
        return None;
    }
    Some(MailTextTarget {
        kind: compose.kind,
        recipient: draft.recipient.clone(),
        draft_epoch: compose.draft_epoch(),
        editor_revision: editor.revision(),
        session_revision,
    })
}

#[cfg(test)]
#[path = "mail_text_adapter_tests.rs"]
mod tests;
