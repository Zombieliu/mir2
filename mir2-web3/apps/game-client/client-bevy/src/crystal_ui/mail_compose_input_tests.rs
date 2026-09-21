use super::*;

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailComposeUi>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<StorageModel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::audio::NativeUiAudioQueue>()
        .add_message::<KeyboardInput>()
        .insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() })
        .add_systems(Update, process_overlay_keyboard);
    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.core.panel = mir2_ui_core::state::UiPanel::Mail;
    state.core.mail_compose = Some(default());
    drop(state);
    app.world_mut().resource_mut::<MailComposeUi>().kind = MailComposeKind::Parcel;
    app
}

fn key(app: &mut App, code: KeyCode, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: bevy::input::keyboard::Key::Character(text.unwrap_or("").into()),
        state: ButtonState::Pressed,
        text: text.map(Into::into), repeat: false, window: Entity::PLACEHOLDER,
    });
    app.update();
}

#[test]
fn compose_enter_keeps_multiline_without_text_payload_or_accidental_send() {
    let mut app = app();
    app.world_mut().resource_mut::<MailComposeUi>().focus = MailComposeFocus::Message;
    key(&mut app, KeyCode::KeyA, Some("First"));
    key(&mut app, KeyCode::Enter, None);
    key(&mut app, KeyCode::KeyB, Some("Second"));
    key(&mut app, KeyCode::NumpadEnter, Some("\r"));
    let state = app.world().resource::<NativePlayerUiState>();
    assert_eq!(state.core.mail_compose.as_ref().unwrap().message, "First\nSecond\n");
    assert!(!state.chat_focused());
    assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents().is_empty());
}

#[test]
fn compose_gold_accepts_digits_backspace_and_rejects_overflow_without_wallet_mutation() {
    let mut app = app();
    app.world_mut().resource_mut::<MailComposeUi>().focus = MailComposeFocus::Gold;
    key(&mut app, KeyCode::Digit1, Some("12x3"));
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap().gold, 123);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Backspace);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap().gold, 12);
    app.world_mut().resource_mut::<NativePlayerUiState>().core.mail_compose.as_mut().unwrap().gold = u32::MAX;
    key(&mut app, KeyCode::Digit9, Some("9"));
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap().gold, u32::MAX);
    assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents().is_empty());
}

#[test]
fn compose_recipient_uses_server_twenty_scalar_limit() {
    let mut app = app();
    key(&mut app, KeyCode::KeyA, Some(&"界".repeat(25)));
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap().recipient, "界".repeat(20));
}
