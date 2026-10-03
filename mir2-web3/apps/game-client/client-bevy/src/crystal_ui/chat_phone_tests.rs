use super::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

fn fixture(bounds: Option<Rect>, cursor: Vec2) -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(CrystalChatPointerBounds(bounds))
        .init_resource::<CrystalChatState>()
        .init_resource::<NativePlayerUiState>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(ChatModel {
            lines: (0..20)
                .map(|i| ChatLine {
                    text: i.to_string(),
                    channel: "normal".into(),
                })
                .collect(),
        })
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .add_message::<MouseWheel>()
        .add_systems(Update, handle_chat_pointer_scroll);
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.resolution.set_scale_factor_override(Some(2.0));
    window.resolution.set(851.0, 393.0);
    window.set_cursor_position(Some(cursor));
    let id = app
        .world_mut()
        .spawn((window, bevy::window::PrimaryWindow))
        .id();
    app.world_mut().resource_mut::<CrystalChatState>().scroll = 10;
    (app, id)
}

fn wheel(app: &mut App, window: Entity) {
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
}

#[test]
fn default_does_not_enable_mobile_pointer_override() {
    assert_eq!(CrystalChatPointerBounds::default().0, None);
}

#[test]
fn phone_wheel_uses_actual_bounds_at_double_density() {
    let bounds = Rect::from_corners(Vec2::new(300.0, 580.0), Vec2::new(1300.0, 740.0));
    let (mut app, window) = fixture(Some(bounds), Vec2::new(200.0, 310.0));
    wheel(&mut app, window);
    assert_eq!(app.world().resource::<CrystalChatState>().scroll, 9);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(20.0, 20.0)));
    wheel(&mut app, window);
    assert_eq!(app.world().resource::<CrystalChatState>().scroll, 9);
}

#[test]
fn hidden_phone_chat_cannot_scroll_or_replay_input() {
    let (mut app, window) = fixture(Some(Rect::EMPTY), Vec2::new(400.0, 350.0));
    wheel(&mut app, window);
    assert_eq!(app.world().resource::<CrystalChatState>().scroll, 10);
    app.world_mut().resource_mut::<CrystalChatPointerBounds>().0 = Some(Rect::from_corners(
        Vec2::new(0.0, 0.0),
        Vec2::new(2000.0, 1000.0),
    ));
    app.update();
    assert_eq!(app.world().resource::<CrystalChatState>().scroll, 10);
}
