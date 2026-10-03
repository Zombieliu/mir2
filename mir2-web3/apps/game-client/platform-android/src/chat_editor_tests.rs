//! The real phone chat child must enter the existing host editor-touch path.
//! No host-global FIFO, server, credential or local chat echo is used here.
use super::*;
use crate::android_input::AndroidShellState;
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
use mir2_client_bevy::crystal_ui::{chat::*, hud::CrystalBeltPresentation};

fn app() -> (App, Entity, Entity) {
    let mut app = App::new();
    let mut player = NativePlayerUiState::default();
    player.core.screen = mir2_ui_core::state::UiScreen::InGame;
    player.set_chat_focused(true);
    player.chat_draft = "retained unsent draft".into();
    app.insert_resource(player)
        .insert_resource(NativeShellModel {
            screen: Screen::InGame,
            ..default()
        })
        .insert_resource(UiScale(0.5))
        .init_resource::<AndroidShellState>()
        .init_resource::<mir2_client_bevy::read_model::UiReadModel>()
        .init_resource::<CrystalChatState>()
        .init_resource::<CrystalBeltPresentation>()
        .init_resource::<CrystalChatPointerBounds>()
        .init_resource::<HostState>()
        .init_resource::<EditorTouch>()
        .add_systems(PreUpdate, remember_editor_touch);
    crate::phone_hud::install(&mut app);
    let mut window = Window::default();
    window.resolution.set_scale_factor_override(Some(1.0));
    window.resolution.set(851.0, 393.0);
    app.world_mut().spawn(window);
    let root = app
        .world_mut()
        .spawn((CrystalChatRoot, Node::default()))
        .id();
    // Same source markers and no Button/NativeTextInputTarget as the shared
    // spawn_chat_input. Interaction lets the test inject a physical UI press;
    // production must install Button as well to receive actual touch focus.
    let input = app
        .world_mut()
        .spawn((
            CrystalChatElement,
            CrystalChatInput,
            Node::default(),
            Interaction::None,
            Text::new("retained unsent draft"),
        ))
        .id();
    let line = app
        .world_mut()
        .spawn((
            CrystalChatElement,
            CrystalChatLine { row: 0 },
            Node::default(),
            Interaction::None,
            Text::new("received, not editable"),
        ))
        .id();
    app.world_mut()
        .entity_mut(root)
        .add_children(&[input, line]);
    app.update();
    (app, input, line)
}

#[test]
fn phone_shared_chat_input_is_a_host_editor_target_without_changing_text_or_authority() {
    let (app, input, line) = app();
    assert!(
        app.world().get::<Button>(input).is_some(),
        "actual touch requires Button"
    );
    assert!(app
        .world()
        .get::<mir2_client_bevy::crystal_ui::overlays::NativeTextInputTarget>(input)
        .is_some());
    assert!(app.world().get::<CrystalChatInput>(input).is_some());
    assert_eq!(
        app.world().get::<Text>(input).unwrap().0,
        "retained unsent draft"
    );
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().chat_draft,
        "retained unsent draft"
    );
    assert!(app.world().get::<CrystalChatAction>(input).is_none());
    assert!(app
        .world()
        .get::<mir2_client_bevy::crystal_ui::overlays::NativeTextInputTarget>(line)
        .is_none());
}

#[test]
fn same_focused_chat_can_reopen_on_a_new_press_but_held_press_and_received_rows_cannot() {
    let (mut app, input, line) = app();
    // Back hides the Android keyboard without changing the shared chat focus.
    // A subsequent tap must generate an editor request even for the same field.
    for (interaction, expect_touch) in [
        (Interaction::Pressed, true),
        (Interaction::Pressed, false),
        (Interaction::None, false),
        (Interaction::Pressed, true),
    ] {
        *app.world_mut().get_mut::<Interaction>(input).unwrap() = interaction;
        app.update();
        assert_eq!(app.world().resource::<EditorTouch>().0, expect_touch);
        let player = app.world().resource::<NativePlayerUiState>();
        assert!(player.chat_focused());
        assert_eq!(crate::text_input::player_field(player).unwrap().0, "chat");
        assert_eq!(player.chat_draft, "retained unsent draft");
    }
    *app.world_mut().get_mut::<Interaction>(input).unwrap() = Interaction::None;
    *app.world_mut().get_mut::<Interaction>(line).unwrap() = Interaction::Pressed;
    app.update();
    assert!(!app.world().resource::<EditorTouch>().0);
}

#[derive(Resource)]
struct RemovePressedInput(Entity);

fn remove_pressed_input(mut commands: Commands, remove: Option<Res<RemovePressedInput>>) {
    if let Some(remove) = remove {
        commands.entity(remove.0).despawn();
        commands.remove_resource::<RemovePressedInput>();
    }
}

#[test]
fn chat_input_press_is_captured_before_shared_children_are_rebuilt() {
    let (mut app, input, _) = app();
    app.add_systems(Update, remove_pressed_input);
    *app.world_mut().get_mut::<Interaction>(input).unwrap() = Interaction::Pressed;
    app.insert_resource(RemovePressedInput(input));
    app.update();
    assert!(app.world().get_entity(input).is_err());
    assert!(
        app.world().resource::<EditorTouch>().0,
        "host must retain the press after Update rebuild"
    );
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().chat_draft,
        "retained unsent draft"
    );
}
