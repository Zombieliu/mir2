use super::*;

#[test]
fn memo_accepts_over_server_limit_and_preserves_draft_for_correction() {
    let mut ui = FriendDialogUi {
        open: true,
        friends: vec![friend(7, false, true)],
        modal: Some(FriendModal::Memo {
            character_index: 7,
            text: "x".repeat(200),
        }),
        ..Default::default()
    };
    ui.sync_editor();
    assert_eq!(ui.paste("\n"), text_editor::EditResult::Changed);
    assert!(ui.modal.is_some());
    assert_eq!(ui.editor.as_ref().unwrap().text().len(), 201);
    assert!(ui.modal_packet().is_none());
    assert!(ui.edit_notice.is_none());
    ui.editor.as_mut().unwrap().select_all();
    ui.paste("中文😀");
    assert!(matches!(ui.modal_packet(),Some(ClientPacket::AddMemo{memo,..}) if memo=="中文😀"));
}

#[test]
fn editor_identity_changes_on_new_modal_and_session_but_not_edits() {
    let mut ui = FriendDialogUi {
        open: true,
        ..Default::default()
    };
    ui.action(FriendAction::Add);
    ui.sync_editor();
    let first = ui.editor_revision;
    ui.paste("Alex");
    ui.sync_editor();
    assert_eq!(first, ui.editor_revision);
    ui.cancel_modal();
    ui.action(FriendAction::Add);
    ui.sync_editor();
    assert_ne!(first, ui.editor_revision);
    let second = ui.editor_revision;
    ui.clear_session();
    ui.show();
    ui.action(FriendAction::Add);
    ui.sync_editor();
    assert_ne!(second, ui.editor_revision);
}

#[test]
fn failed_send_restores_exact_target_and_draft_without_overwriting_new_modal() {
    let mut ui = FriendDialogUi {
        open: true,
        friends: vec![friend(7, false, true)],
        ..Default::default()
    };
    let failed = ClientPacket::AddMemo {
        character_index: 7,
        memo: "draft😀".into(),
    };
    ui.restore_unsent(&failed);
    ui.sync_editor();
    assert!(
        matches!(ui.modal_packet(),Some(ClientPacket::AddMemo{character_index:7,memo}) if memo=="draft😀")
    );
    ui.cancel_modal();
    ui.action(FriendAction::Add);
    ui.paste("New");
    ui.restore_unsent(&failed);
    assert!(matches!(ui.modal_packet(),Some(ClientPacket::AddFriend{name,..}) if name=="New"));
}

#[test]
fn empty_or_oversize_memo_is_never_submitted_to_silent_server_rejection() {
    let mut ui = FriendDialogUi {
        open: true,
        friends: vec![friend(7, false, true)],
        modal: Some(FriendModal::Memo {
            character_index: 7,
            text: String::new(),
        }),
        ..Default::default()
    };
    assert!(ui.modal_packet().is_none());
    *ui.modal_text_mut().unwrap() = "😀".repeat(101);
    assert!(ui.modal_packet().is_none());
    assert!(ui.modal.is_some());
}
fn friend(index: i32, blocked: bool, online: bool) -> ClientFriend {
    ClientFriend {
        index,
        name: format!("Friend{index}"),
        memo: "memo".into(),
        blocked,
        online,
    }
}
#[test]
fn friend_selection_uses_identity_and_update_clears_it() {
    let mut ui = FriendDialogUi::default();
    assert!(matches!(
        ui.show(),
        Some(FriendEffect::Packet(ClientPacket::RefreshFriends))
    ));
    ui.apply_packet(&ServerPacket::FriendUpdate {
        friends: vec![friend(81, false, true), friend(12, false, false)],
    });
    ui.action(FriendAction::Select(1));
    assert_eq!(ui.selected, Some(12));
    ui.action(FriendAction::Remove);
    assert!(matches!(
        ui.modal_packet(),
        Some(ClientPacket::RemoveFriend {
            character_index: 12
        })
    ));
    ui.apply_packet(&ServerPacket::FriendUpdate {
        friends: vec![friend(12, false, true), friend(81, false, true)],
    });
    assert_eq!(ui.selected, None);
    assert!(matches!(
        ui.modal_packet(),
        Some(ClientPacket::RemoveFriend {
            character_index: 12
        })
    ));
}
#[test]
fn friend_removed_target_invalidates_modal() {
    let mut ui = FriendDialogUi {
        open: true,
        friends: vec![friend(12, false, true)],
        ..Default::default()
    };
    ui.action(FriendAction::Select(0));
    ui.action(FriendAction::Memo);
    *ui.modal_text_mut().unwrap() = "unsent draft".into();
    assert!(
        matches!(ui.modal_packet(),Some(ClientPacket::AddMemo { memo,.. }) if memo=="unsent draft")
    );
    assert!(ui.modal.is_some());
    ui.apply_packet(&ServerPacket::FriendUpdate { friends: vec![] });
    assert!(ui.modal.is_none());
}
#[test]
fn friend_tab_and_online_whisper_match_original() {
    let mut ui = FriendDialogUi {
        open: true,
        friends: vec![friend(1, false, false), friend(2, true, true)],
        ..Default::default()
    };
    ui.action(FriendAction::Select(0));
    assert_eq!(
        ui.action(FriendAction::Whisper),
        Some(FriendEffect::Offline)
    );
    ui.action(FriendAction::Blocked);
    ui.action(FriendAction::Select(0));
    assert_eq!(
        ui.action(FriendAction::Whisper),
        Some(FriendEffect::Whisper("/Friend2 ".into()))
    );
    ui.action(FriendAction::Add);
    *ui.modal_text_mut().unwrap() = "NextFriend".into();
    assert!(matches!(
        ui.modal_packet(),
        Some(ClientPacket::AddFriend { blocked: true, .. })
    ));
    ui.clear_session();
    assert!(ui.friends.is_empty());
    assert!(!ui.open);
}
#[test]
fn friend_original_twelve_row_paging_is_preserved() {
    let mut ui = FriendDialogUi {
        open: true,
        friends: (1..=12).map(|i| friend(i, false, true)).collect(),
        ..Default::default()
    };
    assert_eq!(ui.rows().count(), 12);
    assert_eq!(ui.page_count(), 2);
    ui.action(FriendAction::Next);
    assert_eq!(ui.page, 1);
    assert_eq!(ui.start_index, 11);
    assert_eq!(ui.rows().next().unwrap().index, 12);
    ui.action(FriendAction::Previous);
    assert_eq!(ui.start_index, 0);
}
