use super::*;

fn app() -> App {
    let mut app = tests::help_button_test_app();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<UiEffectQueue>()
        .add_message::<KeyboardInput>()
        .add_systems(
            Update,
            process_overlay_keyboard.before(process_overlay_buttons),
        );
    app
}

fn enter(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: bevy::input::keyboard::Key::Enter,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

#[test]
fn leave_game_old_enter_cannot_confirm_new_pointer_prompt() {
    let mut app = app();
    app.update();
    // Winit publishes keyboard input after First's message maintenance.
    // Sending before app.update() would expire it a frame too early here.
    app.add_systems(
        PreUpdate,
        |mut sent: Local<bool>,
         mut keys: ResMut<ButtonInput<KeyCode>>,
         mut messages: MessageWriter<KeyboardInput>| {
            if !*sent {
                *sent = true;
                keys.press(KeyCode::Enter);
                messages.write(KeyboardInput {
                    key_code: KeyCode::Enter,
                    logical_key: bevy::input::keyboard::Key::Enter,
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                });
            }
        },
    );
    let button = app
        .world_mut()
        .spawn((Button, Interaction::Pressed, OverlayButton::ExitApplication))
        .id();
    app.update();
    app.world_mut().despawn(button);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .leave_game
            .prompt,
        Some(leave_game_dialog::LeaveKind::Exit)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .leave_game
            .prompt,
        Some(leave_game_dialog::LeaveKind::Exit),
        "Enter preceding the prompt must not confirm it on the next update"
    );
    assert!(!app
        .world_mut()
        .resource_mut::<UiEffectQueue>()
        .take_exit_application());
    enter(&mut app);
    app.update();
    assert!(
        app.world_mut()
            .resource_mut::<UiEffectQueue>()
            .take_exit_application(),
        "a fresh Enter must still confirm"
    );
}
