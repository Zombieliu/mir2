use super::*;
fn app() -> (App, Entity) {
    let mut ui = NativePlayerUiState::default();
    ui.friends.open = true;
    ui.friends.modal = Some(friend_dialog::FriendModal::Add {
        blocked: false,
        text: "A".into(),
    });
    ui.friends.sync_editor();
    let mut app = App::new();
    app.insert_resource(ui)
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<ImeState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<Ime>()
        .add_systems(Update, process_ime);
    let mut window = Window::default();
    window.focused = true;
    let w = app.world_mut().spawn((window, PrimaryWindow)).id();
    app.update(); // First lease deliberately disables the old OS composition context.
    app.update();
    app.world_mut().write_message(Ime::Enabled { window: w });
    app.update();
    (app, w)
}
fn send(app: &mut App, event: Ime) {
    app.world_mut().write_message(event);
    app.update();
}
#[test]
fn chinese_preedit_is_visible_but_only_commit_updates_the_draft() {
    let (mut app, w) = app();
    send(
        &mut app,
        Ime::Preedit {
            window: w,
            value: "你好".into(),
            cursor: Some((3, 6)),
        },
    );
    let ui = app.world().resource::<NativePlayerUiState>();
    assert_eq!(ui.friends.editor.as_ref().unwrap().text(), "A");
    assert_eq!(ui.friends.display_editor().unwrap().text(), "A你好");
    assert_eq!(ui.friends.display_editor().unwrap().selected_text(), "好");
    assert!(ui.ime_frame_consumed);
    send(
        &mut app,
        Ime::Commit {
            window: w,
            value: "你好".into(),
        },
    );
    let ui = app.world().resource::<NativePlayerUiState>();
    assert_eq!(ui.friends.editor.as_ref().unwrap().text(), "A你好");
    assert!(ui.friends.composition.is_none());
    assert!(
        ui.ime_frame_consumed,
        "Enter delivering the IME commit cannot also submit the modal"
    );
    app.update();
    assert!(
        !app.world()
            .resource::<NativePlayerUiState>()
            .ime_frame_consumed
    );
}
#[test]
fn changed_editor_identity_drops_old_ime_commit() {
    let (mut app, w) = app();
    send(
        &mut app,
        Ime::Preedit {
            window: w,
            value: "旧内容".into(),
            cursor: Some((0, 0)),
        },
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .friends
        .modal = Some(friend_dialog::FriendModal::Memo {
        character_index: 77,
        text: "新目标".into(),
    });
    send(
        &mut app,
        Ime::Commit {
            window: w,
            value: "不能串入".into(),
        },
    );
    let ui = app.world().resource::<NativePlayerUiState>();
    assert_eq!(ui.friends.editor.as_ref().unwrap().text(), "新目标");
    assert!(ui.friends.composition.is_none());
    assert!(!app.world().get::<Window>(w).unwrap().ime_enabled);
}
#[test]
fn escape_cancels_composition_without_closing_the_modal_or_erasing_draft() {
    let (mut app, w) = app();
    send(
        &mut app,
        Ime::Preedit {
            window: w,
            value: "nihao".into(),
            cursor: Some((5, 5)),
        },
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    let ui = app.world().resource::<NativePlayerUiState>();
    assert!(ui.friends.modal.is_some());
    assert!(ui.friends.composition.is_none());
    assert_eq!(ui.friends.editor.as_ref().unwrap().text(), "A");
    assert!(ui.ime_frame_consumed);
}
#[test]
fn focus_loss_or_exact_keyboard_capture_disables_ime_and_rejects_commit() {
    let (mut app, w) = app();
    app.world_mut().get_mut::<Window>(w).unwrap().focused = false;
    send(
        &mut app,
        Ime::Commit {
            window: w,
            value: "不能串入".into(),
        },
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .friends
            .editor
            .as_ref()
            .unwrap()
            .text(),
        "A"
    );
    assert!(!app.world().get::<Window>(w).unwrap().ime_enabled);
    app.world_mut().get_mut::<Window>(w).unwrap().focused = true;
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .keyboard
        .open = true;
    app.update();
    assert!(editor_owner(app.world().resource::<NativePlayerUiState>()).is_none());
    assert!(!app.world().get::<Window>(w).unwrap().ime_enabled);
}
#[test]
fn committed_chinese_obeys_real_name_utf16_limit_and_memo_multiline_policy() {
    let (mut app, w) = app();
    send(
        &mut app,
        Ime::Commit {
            window: w,
            value: "界".repeat(60),
        },
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .friends
            .editor
            .as_ref()
            .unwrap()
            .utf16_len(),
        50
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .friends
        .modal = Some(friend_dialog::FriendModal::Memo {
        character_index: 7,
        text: String::new(),
    });
    app.update();
    app.update();
    send(&mut app, Ime::Enabled { window: w });
    send(
        &mut app,
        Ime::Commit {
            window: w,
            value: "第一行\r\n第二行".into(),
        },
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .friends
            .editor
            .as_ref()
            .unwrap()
            .text(),
        "第一行\n第二行"
    );
}

#[test]
fn prompt_owner_matches_keyboard_dispatch_above_covered_key_configuration() {
    let mut ui = NativePlayerUiState::default();
    ui.keyboard.open = true;
    ui.group_dialog.editor.open = true;
    ui.group_dialog.editor.modal = Some(friend_dialog::FriendModal::Add {
        blocked: false,
        text: String::new(),
    });
    ui.group_dialog.editor.sync_editor();
    assert_eq!(editor_owner(&ui), Some(EditorOwner::Group));
    ui.social_bonds.prompt = Some(social_bond_dialog::BondPrompt {
        revision: 1,
        kind: social_bond_dialog::BondPromptKind::MentorName {
            text: String::new(),
        },
    });
    ui.social_bonds.sync_editor();
    ui.group_dialog.invitation = Some(("Other".into(), 1));
    assert_eq!(editor_owner(&ui), Some(EditorOwner::Bond));
    ui.social_bonds.prompt = None;
    assert_eq!(editor_owner(&ui), None);
    ui.group_dialog.invitation = None;
    assert_eq!(editor_owner(&ui), Some(EditorOwner::Group));
    ui.group_dialog.editor.cancel_modal();
    assert_eq!(editor_owner(&ui), None);
}
#[test]
fn menu_editor_and_overlay_labels_use_original_arial_family() {
    let mut app = App::new();
    let mut ui = NativePlayerUiState::default();
    ui.friends.open = true;
    ui.friends.modal = Some(friend_dialog::FriendModal::Add {
        blocked: false,
        text: "中文".into(),
    });
    ui.friends.sync_editor();
    app.insert_resource(ui);
    app.add_systems(
        Startup,
        |mut commands: Commands, ui: Res<NativePlayerUiState>| {
            commands.spawn(Node::default()).with_children(|p| {
                friend_dialog::view::render_editor(
                    p,
                    &ui.friends,
                    CrystalRect::new(0., 0., 240., 19.),
                    false,
                );
                overlay_text_at(
                    p,
                    "Guild",
                    CrystalRect::new(0., 20., 80., 20.),
                    32. / 3.,
                    Color::WHITE,
                );
            });
        },
    );
    app.update();
    let fonts = app
        .world_mut()
        .query::<&TextFont>()
        .iter(app.world())
        .collect::<Vec<_>>();
    assert_eq!(fonts.len(), 2);
    for font in fonts {
        assert_eq!(font.font, bevy::prelude::FontSource::Family("Arial".into()));
    }
}
