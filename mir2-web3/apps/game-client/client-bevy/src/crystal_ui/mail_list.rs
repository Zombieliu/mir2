//! Native compatibility registration and regression tests; all row painting is shared.
use super::*;
pub(super) use super::super::mail_page_shared::layout_mail_row_icons as layout_icons;
#[cfg(test)]
use super::super::mail_page_shared::preview;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_preview_uses_message_not_subject_and_updates_lock_marker() {
        let mut message = MailMessage { subject: "not the preview".into(), body: "First\r\nSecond".into(), ..default() };
        assert_eq!(preview(&message), "First Second");
        message.locked = true;
        assert_eq!(preview(&message), "[*] First Second");
    }

    #[test]
    fn list_uses_original_rows_and_hides_unavailable_reply_and_list_claim() {
        let mut app = super::super::tests::overlay_render_test_app();
        app.world_mut().resource_mut::<NativePlayerUiState>().core.panel = mir2_ui_core::state::UiPanel::Mail;
        {
            let mut mail = app.world_mut().resource_mut::<MailModel>();
            mail.mails.push(MailMessage { id: 5, sender: "Sender".into(), body: "Preview".into(), gold: 100, locked: true, ..default() });
            mail.selected_id = Some(5);
        }
        app.update();
        let world = app.world_mut();
        let paths: Vec<_> = world.query::<&ImageNode>().iter(world)
            .filter_map(|image| image.image.path().map(|path| path.to_string())).collect();
        for path in ["Title/7", "Prguse/541", "Prguse/545", "Prguse/550", "Prguse/551", "Prguse/552", "Prguse/520", "Prguse/523"] {
            assert!(paths.contains(&format!("original-ui/{path}.png")), "missing {path}");
        }
        let buttons: Vec<_> = world.query::<&OverlayButton>().iter(world).collect();
        assert!(!buttons.iter().any(|button| matches!(button, OverlayButton::MailReply(_) | OverlayButton::ClaimMail(_))));
        let (node, _) = world.query::<(&Node, &OverlayButton)>().iter(world)
            .find(|(_, button)| matches!(button, OverlayButton::SelectMail(5))).unwrap();
        assert_eq!((node.left, node.top, node.width, node.height), (Val::Px(10.0), Val::Px(55.0), Val::Px(290.0), Val::Px(33.0)));
    }
}
