//! Chat controls are drawn by the shared renderer, but the native world sender
//! runs before Bevy updates Interaction. Exercise that sender with the previous
//! completed UI layout and real ordered mouse edges, not a pre-set UI blocker.
use super::tests::{
    ordered_pointer_button, ordered_pointer_fixture, ordered_pointer_motion, quest_pointer_test_app,
};
use super::*;
use bevy::prelude::{App, Vec2};
use mir2_client_bevy::crystal_ui::chat::{chat_control_bar_rect, CrystalChatWindowSize};

fn painted_chat(app: &mut App, size: CrystalChatWindowSize, scale: f32, visible: bool) {
    let visibility = if visible {
        InheritedVisibility::VISIBLE
    } else {
        InheritedVisibility::HIDDEN
    };
    let root = app
        .world_mut()
        .spawn((
            CrystalChatRoot,
            Node::default(),
            ComputedNode {
                size: Vec2::new(1024., 768.) * scale,
                ..Default::default()
            },
            UiGlobalTransform::from_translation(Vec2::new(512., 384.) * scale),
            visibility,
        ))
        .id();
    // These are the renderer's painted bar and current chat frame, rather than
    // its full-stage transparent positioning root.
    for rect in [chat_control_bar_rect(size), size.spec_rect()] {
        let child = app
            .world_mut()
            .spawn((
                Node::default(),
                ImageNode::default(),
                ComputedNode {
                    size: Vec2::new(rect.width, rect.height) * scale,
                    ..Default::default()
                },
                UiGlobalTransform::from_translation(
                    Vec2::new(rect.center().0, rect.center().1) * scale,
                ),
                visibility,
            ))
            .id();
        app.world_mut().entity_mut(root).add_child(child);
    }
}

fn resize_window(app: &mut App, cursor: Vec2, scale: f32) {
    let world = app.world_mut();
    let mut window = world.query::<&mut Window>().single_mut(world).unwrap();
    window.resolution.set(1024. * scale, 768. * scale);
    window.set_cursor_position(Some(cursor * scale));
}

#[test]
fn native_chat_pointer_trade_filters_and_resize_never_emit_world_commands() {
    for size in [
        CrystalChatWindowSize::Small,
        CrystalChatWindowSize::Medium,
        CrystalChatWindowSize::Large,
    ] {
        let bar = chat_control_bar_rect(size);
        for scale in [1., 1.5] {
            // All seven channel buttons, Trade, resize and chat settings.
            for left in [12., 34., 56., 78., 100., 122., 144., 166., 574., 596.] {
                for button in [MouseButton::Left, MouseButton::Right] {
                    let cursor = Vec2::new(bar.left + left + 5., bar.top + 6.);
                    let (mut app, receiver) = quest_pointer_test_app(cursor);
                    resize_window(&mut app, cursor, scale);
                    painted_chat(&mut app, size, scale, true);
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .press(button);
                    app.update();
                    assert!(
                        receiver.try_recv().is_err(),
                        "{size:?} {scale} {left} {button:?} leaked to world"
                    );
                    assert!(app
                        .world_mut()
                        .resource_mut::<QuestUiIntentQueue>()
                        .drain_intents()
                        .is_empty());
                    let movement = app.world().resource::<WorldPointerMovementState>();
                    assert!(movement.active.is_none() && movement.pending.is_empty());
                }
            }
        }
    }
}

#[test]
fn native_chat_pointer_root_does_not_mask_world_or_hidden_chat() {
    for (cursor, visible) in [
        (Vec2::new(650., 450.), true),
        (Vec2::new(401., 663.), false),
    ] {
        let (mut app, receiver) = quest_pointer_test_app(cursor);
        painted_chat(&mut app, CrystalChatWindowSize::Small, 1., visible);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { .. }))
        ));
    }
}

#[test]
fn native_chat_pointer_down_then_drag_to_world_cannot_start_movement() {
    for button in [MouseButton::Left, MouseButton::Right] {
        for new_move in [false, true] {
            let (mut app, receiver) = ordered_pointer_fixture();
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .options
                .new_move = new_move;
            painted_chat(&mut app, CrystalChatWindowSize::Small, 1., true);
            ordered_pointer_motion(&mut app, Vec2::new(401., 663.));
            ordered_pointer_button(&mut app, button, true);
            ordered_pointer_motion(&mut app, Vec2::new(650., 450.));
            app.update();
            assert!(
                receiver.try_recv().is_err(),
                "chat press escaped into world before release"
            );
            ordered_pointer_button(&mut app, button, false);
            app.update();
            assert!(receiver.try_recv().is_err());
            assert!(app
                .world()
                .resource::<WorldPointerMovementState>()
                .active
                .is_none());
        }
    }
}

#[test]
fn native_chat_pointer_hover_leaves_keyboard_movement_available() {
    let (mut app, receiver) = quest_pointer_test_app(Vec2::new(401., 663.));
    painted_chat(&mut app, CrystalChatWindowSize::Small, 1., true);
    app.add_systems(bevy::app::Update, keyboard_movement_system);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        // D is an original UI shortcut when the real player UI is installed.
        // Use the unbound directional key to test keyboard movement itself.
        .press(KeyCode::ArrowRight);
    app.update();
    assert!(
        matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right")
    );
}
