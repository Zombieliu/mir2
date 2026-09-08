//! Android touch affordances over the existing shared player UI state.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::overlays::NativePlayerUiState,
    native_shell::{NativeShellModel, NativeShellScreen},
};

#[derive(Component, Clone, Copy)]
enum Action {
    Panels,
    Bag,
    Character,
    Skills,
    Quests,
    Mail,
    Options,
    Menu,
    Chat,
    Close,
}
#[derive(Component)]
struct TouchRail;
#[derive(Component)]
struct RailLabel;
#[derive(Resource, Default)]
struct RailState {
    expanded: bool,
}
#[derive(Resource, Default)]
struct TouchPointer {
    owner: Option<u64>,
    wait_for_release: bool,
}
pub fn install(app: &mut App) {
    app.init_resource::<RailState>()
        .init_resource::<TouchPointer>()
        .add_systems(Startup, spawn)
        .add_systems(Update, buttons)
        .add_systems(PreUpdate, touch_pointer.after(bevy::input::InputSystems))
        .add_systems(
            PostUpdate,
            visibility.after(super::shared_shell::AndroidStageFit),
        );
}

// Bevy UI already handles touch clicks. Source Crystal drag/scroll handlers
// additionally read Window cursor + left-button state, so bridge one owning
// finger to those SAME handlers rather than implementing new item operations.
fn touch_pointer(
    touches: Option<Res<Touches>>,
    mut pointer: ResMut<TouchPointer>,
    mut windows: Query<&mut Window>,
    mut mouse: Option<ResMut<ButtonInput<MouseButton>>>,
) {
    let (Some(touches), Some(mouse), Ok(mut window)) =
        (touches, mouse.as_deref_mut(), windows.single_mut())
    else {
        return;
    };
    if !window.focused {
        if pointer.owner.take().is_some() {
            mouse.release(MouseButton::Left);
        }
        pointer.wait_for_release = true;
        return;
    }
    if pointer.wait_for_release {
        pointer.wait_for_release = touches.iter().next().is_some();
        return;
    }
    if let Some(owner) = pointer.owner {
        if let Some(touch) = touches.get_pressed(owner) {
            window.set_cursor_position(Some(touch.position()));
        } else {
            mouse.release(MouseButton::Left);
            pointer.owner = None;
            pointer.wait_for_release = touches.iter().next().is_some();
        }
    } else if let Some(touch) = touches.iter_just_pressed().min_by_key(|touch| touch.id()) {
        pointer.owner = Some(touch.id());
        window.set_cursor_position(Some(touch.position()));
        mouse.press(MouseButton::Left);
    }
}
fn spawn(mut commands: Commands) {
    commands
        .spawn((
            TouchRail,
            GlobalZIndex(9500),
            Node {
                position_type: PositionType::Absolute,
                right: px(8),
                top: px(80),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(6),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|rail| {
            for (label, action) in [
                ("Panels", Action::Panels),
                ("Bag", Action::Bag),
                ("Char", Action::Character),
                ("Skills", Action::Skills),
                ("Quests", Action::Quests),
                ("Mail", Action::Mail),
                ("Options", Action::Options),
                ("Menu", Action::Menu),
                ("Chat", Action::Chat),
                ("Close", Action::Close),
            ] {
                rail.spawn((
                    Button,
                    action,
                    Node {
                        width: px(104),
                        height: px(48),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.08, 0.04, 0.95)),
                    BorderColor::all(Color::srgb(0.65, 0.49, 0.22)),
                ))
                .with_children(|button| {
                    button.spawn((
                        RailLabel,
                        Text::new(label),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.85, 0.62)),
                    ));
                });
            }
        });
}
fn visibility(
    shell: Res<NativeShellModel>,
    scale: Res<UiScale>,
    mut rail: ResMut<RailState>,
    mut roots: Query<&mut Node, (With<TouchRail>, Without<Action>)>,
    mut buttons: Query<(&Action, &mut Node), Without<TouchRail>>,
    mut labels: Query<&mut TextFont, With<RailLabel>>,
) {
    let unit = 1.0 / scale.0.max(0.01);
    if shell.screen != NativeShellScreen::InGame {
        rail.expanded = false;
    }
    for mut node in &mut roots {
        node.width = px(if rail.expanded { 132.0 } else { 64.0 } * unit);
        node.top = px(48.0 * unit);
        node.right = px(8.0 * unit);
        node.row_gap = px(4.0 * unit);
        node.column_gap = px(4.0 * unit);
        node.display = if shell.screen == NativeShellScreen::InGame {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (action, mut node) in &mut buttons {
        node.width = px(64.0 * unit);
        node.height = px(48.0 * unit);
        node.display = if rail.expanded || matches!(action, Action::Panels) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut font in &mut labels {
        font.font_size = FontSize::Px(12.0 * unit);
    }
}
fn buttons(
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
    mut rail: ResMut<RailState>,
    mut buttons: Query<(&Interaction, &Action, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, action, mut background) in &mut buttons {
        background.0 = if *interaction == Interaction::Pressed {
            Color::srgb(0.35, 0.25, 0.10)
        } else {
            Color::srgba(0.10, 0.08, 0.04, 0.95)
        };
        if *interaction != Interaction::Pressed || shell.screen != NativeShellScreen::InGame {
            continue;
        }
        // A modal retains exclusive ownership; only its own controls may confirm it.
        if state.amount_modal_open() {
            continue;
        }
        match action {
            Action::Panels => {
                rail.expanded = !rail.expanded;
                continue;
            }
            Action::Bag => state.toggle_inventory(),
            Action::Character => state.toggle_equipment(),
            Action::Skills => state.toggle_skill(),
            Action::Quests => state.toggle_quest(),
            Action::Mail => state.toggle_mail(),
            Action::Options => state.toggle_options(),
            Action::Menu => state.toggle_menu(),
            Action::Chat => {
                let focused = state.chat_focused();
                state.set_chat_focused(!focused);
            }
            Action::Close => state.close_windows(),
        }
        rail.expanded = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn touch_targets_keep_48_logical_pixels_after_stage_scaling() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(UiScale(0.5))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .init_resource::<NativePlayerUiState>();
        install(&mut app);
        app.update();
        let mut query = app.world_mut().query::<(&Action, &Node)>();
        let mut visible = 0;
        for (_, node) in query.iter(app.world()) {
            assert_eq!(node.height, px(96));
            if node.display != Display::None {
                visible += 1;
            }
        }
        assert_eq!(visible, 1, "rail is collapsed by default");
    }
}
