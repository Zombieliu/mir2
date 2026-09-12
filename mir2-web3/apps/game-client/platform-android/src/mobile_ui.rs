//! Android touch affordances over the existing shared player UI state.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::overlays::{
        dispatch_ui_action, NativePlayerUiSet, NativePlayerUiState, UiEffectQueue,
    },
    native_shell::{NativeShellModel, NativeShellScreen},
    quest_model::{CombatTargetModel, GroundPickupModel},
    quest_ui::{QuestUiIntent, QuestUiIntentQueue},
    read_model::UiReadModel,
};

use crate::android_input::{AndroidInputEvent, AndroidInputMessage, AndroidShellState};

const JOYSTICK_DIAMETER: f32 = 144.0;
const JOYSTICK_KNOB_DIAMETER: f32 = 56.0;
const JOYSTICK_TRAVEL: f32 = 42.0;
const JOYSTICK_LEFT: f32 = 24.0;
const JOYSTICK_BOTTOM: f32 = 88.0;
const JOYSTICK_EMIT_SECONDS: f64 = 0.1;

#[derive(Component, Clone, Copy)]
enum Action {
    Attack,
    RunToggle,
    Panels,
    Pickup,
    Revive,
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
struct RailButton;
#[derive(Component)]
struct ActionPad;
#[derive(Component)]
struct ActionPadButton;
#[derive(Component)]
struct JoystickRoot;
#[derive(Component)]
struct JoystickKnob;
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
#[derive(Resource)]
struct JoystickState {
    owner: Option<u64>,
    vector: Vec2,
    run_lock: bool,
    last_emit_at: Option<f64>,
    claimed_this_frame: Vec<u64>,
}

impl Default for JoystickState {
    fn default() -> Self {
        Self {
            owner: None,
            vector: Vec2::ZERO,
            run_lock: true,
            last_emit_at: None,
            claimed_this_frame: Vec::new(),
        }
    }
}

impl JoystickState {
    fn claims(&self, id: u64) -> bool {
        self.owner == Some(id) || self.claimed_this_frame.contains(&id)
    }

    fn release(&mut self) {
        self.owner = None;
        self.vector = Vec2::ZERO;
        self.last_emit_at = None;
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<RailState>()
        .init_resource::<UiEffectQueue>()
        .init_resource::<TouchPointer>()
        .init_resource::<JoystickState>()
        .add_systems(Startup, spawn)
        .add_systems(
            Update,
            (buttons, keep_drag_handles_reachable)
                .chain()
                .after(NativePlayerUiSet::Mutate)
                .before(NativePlayerUiSet::Read),
        )
        .add_systems(
            PreUpdate,
            (joystick_touch, touch_pointer)
                .chain()
                .after(bevy::input::InputSystems)
                .before(bevy::ui::UiSystems::Focus),
        )
        .add_systems(
            PostUpdate,
            (visibility, joystick_visual)
                .chain()
                .after(super::shared_shell::AndroidStageFit)
                .before(bevy::ui::UiSystems::Layout),
        );
}

// Bevy UI already handles touch clicks. Source Crystal drag/scroll handlers
// additionally read Window cursor + left-button state, so bridge one owning
// finger to those SAME handlers rather than implementing new item operations.
fn joystick_touch(
    touches: Option<Res<Touches>>,
    time: Option<Res<Time>>,
    shell: Res<NativeShellModel>,
    android: Option<Res<AndroidShellState>>,
    windows: Query<&Window>,
    mut joystick: ResMut<JoystickState>,
    mut input: Option<MessageWriter<AndroidInputMessage>>,
) {
    joystick.claimed_this_frame.clear();
    let (Some(touches), Ok(window)) = (touches, windows.single()) else {
        joystick.release();
        return;
    };
    if shell.screen != NativeShellScreen::InGame || !window.focused {
        joystick.release();
        return;
    }

    let dpi = window.scale_factor().max(0.01);
    let safe_left = android
        .as_deref()
        .map(|state| state.safe_area.left / dpi)
        .unwrap_or(0.0);
    let safe_bottom = android
        .as_deref()
        .map(|state| state.safe_area.bottom / dpi)
        .unwrap_or(0.0);
    let center = joystick_center(window, safe_left, safe_bottom);
    if joystick.owner.is_none() {
        joystick.owner = touches
            .iter_just_pressed()
            .filter(|touch| touch.position().distance(center) <= JOYSTICK_DIAMETER * 0.62)
            .min_by_key(|touch| touch.id())
            .map(|touch| touch.id());
    }
    let Some(owner) = joystick.owner else {
        joystick.vector = Vec2::ZERO;
        joystick.last_emit_at = None;
        return;
    };
    joystick.claimed_this_frame.push(owner);

    let (position, released) = if let Some(touch) = touches.get_pressed(owner) {
        (touch.position(), false)
    } else if let Some(touch) = touches.get_released(owner) {
        (touch.position(), true)
    } else {
        joystick.release();
        return;
    };
    joystick.vector = ((position - center) / JOYSTICK_TRAVEL).clamp_length_max(1.0);
    let now = time.as_deref().map(Time::elapsed_secs_f64).unwrap_or(0.0);
    let should_emit = joystick.vector.length_squared() >= 0.15 * 0.15
        && joystick
            .last_emit_at
            .is_none_or(|last| now - last >= JOYSTICK_EMIT_SECONDS);
    if should_emit {
        if let Some(input) = input.as_mut() {
            input.write(AndroidInputMessage(AndroidInputEvent::VirtualJoystick {
                x: joystick.vector.x,
                y: joystick.vector.y,
                run: joystick.run_lock,
            }));
        }
        joystick.last_emit_at = Some(now);
    }
    if released {
        joystick.release();
        joystick.claimed_this_frame.push(owner);
    }
}

fn joystick_center(window: &Window, safe_left: f32, safe_bottom: f32) -> Vec2 {
    Vec2::new(
        safe_left + JOYSTICK_LEFT + JOYSTICK_DIAMETER * 0.5,
        window.height() - safe_bottom - JOYSTICK_BOTTOM - JOYSTICK_DIAMETER * 0.5,
    )
}

fn touch_pointer(
    touches: Option<Res<Touches>>,
    joystick: Option<Res<JoystickState>>,
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
        window.set_cursor_position(None);
        return;
    }
    if pointer.wait_for_release {
        pointer.wait_for_release = touches.iter().any(|touch| {
            !joystick
                .as_deref()
                .is_some_and(|state| state.claims(touch.id()))
        });
        return;
    }
    if let Some(owner) = pointer.owner {
        if let Some(touch) = touches.get_pressed(owner) {
            window.set_cursor_position(Some(touch.position()));
        } else {
            if let Some(touch) = touches.get_released(owner) {
                window.set_cursor_position(Some(touch.position()));
            }
            mouse.release(MouseButton::Left);
            pointer.owner = None;
            pointer.wait_for_release = touches.iter().any(|touch| {
                !joystick
                    .as_deref()
                    .is_some_and(|state| state.claims(touch.id()))
            });
        }
    } else if let Some(touch) = touches
        .iter_just_pressed()
        .filter(|touch| {
            !joystick
                .as_deref()
                .is_some_and(|state| state.claims(touch.id()))
        })
        .min_by_key(|touch| touch.id())
    {
        pointer.owner = Some(touch.id());
        window.set_cursor_position(Some(touch.position()));
        mouse.press(MouseButton::Left);
        // A quick tap can start and end within one frame. Keep both edges for
        // the shared handlers, without synthesizing an extra frame of holding.
        if touches.get_pressed(touch.id()).is_none() {
            mouse.release(MouseButton::Left);
            pointer.owner = None;
            pointer.wait_for_release = touches.iter().any(|touch| {
                !joystick
                    .as_deref()
                    .is_some_and(|state| state.claims(touch.id()))
            });
        }
    }
}

fn keep_drag_handles_reachable(
    scale: Res<UiScale>,
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
) {
    if shell.screen != NativeShellScreen::InGame {
        return;
    }
    // Do not intercept Android's system gesture. Move only the local movable
    // window positions, leaving the shared stage/hit-test transform unchanged.
    let gutter = (24.0 / scale.0.max(0.01)).min(100.0);
    // Avoid marking the whole shared UI changed when the clamp is a no-op:
    // chat rebuilds on that signal and would lose its button interactions.
    if state.inventory_window.top < gutter {
        state.inventory_window.top = gutter;
    }
    if state.help.top < gutter {
        state.help.top = gutter;
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
                ("Revive", Action::Revive),
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
                    RailButton,
                    action,
                    Node {
                        width: px(104),
                        height: px(48),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(6)),
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

    commands
        .spawn((
            JoystickRoot,
            GlobalZIndex(9400),
            Node {
                position_type: PositionType::Absolute,
                width: px(JOYSTICK_DIAMETER),
                height: px(JOYSTICK_DIAMETER),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.46)),
            BorderColor::all(Color::srgba(0.78, 0.63, 0.34, 0.72)),
        ))
        .with_children(|base| {
            base.spawn((
                JoystickKnob,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(JOYSTICK_KNOB_DIAMETER),
                    height: px(JOYSTICK_KNOB_DIAMETER),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.48, 0.31, 0.12, 0.90)),
                BorderColor::all(Color::srgb(0.98, 0.82, 0.48)),
            ));
        });

    commands
        .spawn((
            ActionPad,
            GlobalZIndex(9500),
            Node {
                position_type: PositionType::Absolute,
                width: px(144),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::End,
                row_gap: px(8),
                column_gap: px(8),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|pad| {
            for (label, action) in [
                ("Attack", Action::Attack),
                ("Pick", Action::Pickup),
                ("Run", Action::RunToggle),
                ("Menu", Action::Panels),
            ] {
                pad.spawn((
                    Button,
                    ActionPadButton,
                    action,
                    Node {
                        width: px(68),
                        height: px(56),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(7)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.08, 0.04, 0.88)),
                    BorderColor::all(Color::srgb(0.70, 0.53, 0.24)),
                ))
                .with_children(|button| {
                    button.spawn((
                        RailLabel,
                        Text::new(label),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.98, 0.88, 0.64)),
                    ));
                });
            }
        });
}
fn visibility(
    shell: Res<NativeShellModel>,
    state: Res<NativePlayerUiState>,
    ui: Option<Res<UiReadModel>>,
    pickups: Option<Res<GroundPickupModel>>,
    scale: Res<UiScale>,
    host: Option<Res<crate::shared_shell::HostState>>,
    android: Option<Res<AndroidShellState>>,
    windows: Query<&Window>,
    mut rail: ResMut<RailState>,
    mut rail_roots: Query<&mut Node, With<TouchRail>>,
    mut rail_buttons: Query<(&Action, &mut Node), (With<RailButton>, Without<TouchRail>)>,
    mut pad_roots: Query<&mut Node, (With<ActionPad>, Without<TouchRail>, Without<RailButton>)>,
    mut pad_buttons: Query<
        &mut Node,
        (
            With<ActionPadButton>,
            Without<TouchRail>,
            Without<RailButton>,
            Without<ActionPad>,
        ),
    >,
    mut joystick_roots: Query<
        &mut Node,
        (
            With<JoystickRoot>,
            Without<TouchRail>,
            Without<RailButton>,
            Without<ActionPad>,
            Without<ActionPadButton>,
        ),
    >,
    mut labels: Query<&mut TextFont, With<RailLabel>>,
) {
    let unit = 1.0 / scale.0.max(0.01);
    let dpi = windows
        .single()
        .map(|window| window.scale_factor())
        .unwrap_or(1.0);
    let safe_top = host.as_ref().map(|h| h.safe_top / dpi).unwrap_or(0.0);
    let safe_right = host.as_ref().map(|h| h.safe_right / dpi).unwrap_or(0.0);
    let safe_left = android
        .as_deref()
        .map(|state| state.safe_area.left / dpi)
        .unwrap_or(0.0);
    let safe_bottom = android
        .as_deref()
        .map(|state| state.safe_area.bottom / dpi)
        .unwrap_or(0.0);
    let expanded_map = mir2_client_bevy::crystal_ui::hud::minimap_is_expanded(
        state.minimap_visible(),
        ui.as_ref().and_then(|ui| ui.player.map_name.as_deref()),
    );
    let pickup_available = pickups
        .as_deref()
        .and_then(|pickups| pickups.recent.front())
        .and_then(|pickup| pickup.object_id)
        .is_some();
    let map_bottom = mir2_client_bevy::crystal_ui::hud::minimap_footer_top(expanded_map) + 20.0;
    if shell.screen != NativeShellScreen::InGame {
        rail.expanded = false;
    }
    let in_game = shell.screen == NativeShellScreen::InGame;
    for mut node in &mut rail_roots {
        node.width = px(if rail.expanded { 132.0 } else { 64.0 } * unit);
        node.top = px((safe_top + 16.0 + map_bottom * scale.0).max(48.0) * unit);
        node.right = px((safe_right + 8.0) * unit);
        node.row_gap = px(4.0 * unit);
        node.column_gap = px(4.0 * unit);
        node.display = if in_game {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (action, mut node) in &mut rail_buttons {
        node.width = px(64.0 * unit);
        node.height = px(48.0 * unit);
        let visible = if matches!(action, Action::Pickup) {
            pickup_available
        } else if matches!(action, Action::Revive) {
            can_request_revive(&shell, &state, ui.as_deref())
        } else {
            rail.expanded || matches!(action, Action::Panels)
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut pad_roots {
        node.width = px(144.0 * unit);
        node.right = px((safe_right + 20.0) * unit);
        node.bottom = px((safe_bottom + JOYSTICK_BOTTOM) * unit);
        node.row_gap = px(8.0 * unit);
        node.column_gap = px(8.0 * unit);
        node.display = if in_game {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut pad_buttons {
        node.width = px(68.0 * unit);
        node.height = px(56.0 * unit);
        node.display = Display::Flex;
    }
    for mut node in &mut joystick_roots {
        node.width = px(JOYSTICK_DIAMETER * unit);
        node.height = px(JOYSTICK_DIAMETER * unit);
        node.left = px((safe_left + JOYSTICK_LEFT) * unit);
        node.bottom = px((safe_bottom + JOYSTICK_BOTTOM) * unit);
        node.border = UiRect::all(px(2.0 * unit));
        node.display = if in_game {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut font in &mut labels {
        font.font_size = FontSize::Px(12.0 * unit);
    }
}

fn joystick_visual(
    scale: Res<UiScale>,
    joystick: Res<JoystickState>,
    mut knobs: Query<&mut Node, With<JoystickKnob>>,
) {
    let unit = 1.0 / scale.0.max(0.01);
    let offset = joystick.vector * JOYSTICK_TRAVEL;
    let centered = (JOYSTICK_DIAMETER - JOYSTICK_KNOB_DIAMETER) * 0.5;
    for mut node in &mut knobs {
        node.width = px(JOYSTICK_KNOB_DIAMETER * unit);
        node.height = px(JOYSTICK_KNOB_DIAMETER * unit);
        node.left = px((centered + offset.x) * unit);
        node.top = px((centered + offset.y) * unit);
        node.border = UiRect::all(px(2.0 * unit));
    }
}
fn buttons(
    shell: Res<NativeShellModel>,
    ui: Option<Res<UiReadModel>>,
    target: Option<Res<CombatTargetModel>>,
    pickups: Option<Res<GroundPickupModel>>,
    mut quest_intents: Option<ResMut<QuestUiIntentQueue>>,
    mut effects: Option<ResMut<UiEffectQueue>>,
    mut state: ResMut<NativePlayerUiState>,
    mut rail: ResMut<RailState>,
    mut joystick: Option<ResMut<JoystickState>>,
    mut buttons: Query<(&Interaction, &Action, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, action, mut background) in &mut buttons {
        background.0 = if *interaction == Interaction::Pressed {
            Color::srgb(0.35, 0.25, 0.10)
        } else if matches!(action, Action::RunToggle)
            && joystick.as_deref().is_some_and(|state| state.run_lock)
        {
            Color::srgba(0.42, 0.27, 0.08, 0.94)
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
            Action::Attack => {
                if let (Some(object_id), Some(queue)) = (
                    target
                        .as_deref()
                        .and_then(|target| target.target.as_ref())
                        .filter(|target| !target.is_dead())
                        .map(|target| target.object_id),
                    quest_intents.as_deref_mut(),
                ) {
                    let _ = queue.push_intent(QuestUiIntent::AttackTarget { object_id });
                    #[cfg(feature = "ui-preview")]
                    info!(
                        "ANDROID_UI_PREVIEW_INTENT attack object_id={object_id} (queued only, no server)"
                    );
                }
            }
            Action::RunToggle => {
                if let Some(joystick) = joystick.as_deref_mut() {
                    joystick.run_lock = !joystick.run_lock;
                    background.0 = if joystick.run_lock {
                        Color::srgba(0.42, 0.27, 0.08, 0.94)
                    } else {
                        Color::srgba(0.10, 0.08, 0.04, 0.95)
                    };
                }
            }
            Action::Pickup => {
                if let (Some(object_id), Some(queue)) = (
                    pickups
                        .as_deref()
                        .and_then(|pickups| pickups.recent.front())
                        .and_then(|pickup| pickup.object_id),
                    quest_intents.as_deref_mut(),
                ) {
                    let _ = queue.push_intent(QuestUiIntent::PickUpObject { object_id });
                    #[cfg(feature = "ui-preview")]
                    info!(
                        "ANDROID_UI_PREVIEW_INTENT pickUp object_id={object_id} (queued only, no server)"
                    );
                }
            }
            Action::Revive => {
                if can_request_revive(&shell, &state, ui.as_deref()) {
                    if let Some(effects) = effects.as_deref_mut() {
                        dispatch_ui_action(
                            &mut state.core,
                            effects,
                            mir2_ui_core::action::UiAction::TownRevive,
                        );
                        #[cfg(feature = "ui-preview")]
                        info!("ANDROID_UI_PREVIEW_INTENT townRevive (queued only, no server)");
                    }
                }
            }
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
            Action::Close => state.close_all_windows(),
        }
        rail.expanded = false;
    }
}

fn can_request_revive(
    shell: &NativeShellModel,
    state: &NativePlayerUiState,
    ui: Option<&UiReadModel>,
) -> bool {
    shell.screen == NativeShellScreen::InGame
        && state.core.screen == mir2_ui_core::state::UiScreen::InGame
        && !state.amount_modal_open()
        && ui.is_some_and(|ui| ui.player.max_hp > 0 && ui.player.hp <= 0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn settled_drag_positions_do_not_invalidate_shared_ui_each_frame() {
        let mut app = App::new();
        app.insert_resource(UiScale(0.5))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .add_systems(Update, keep_drag_handles_reachable);
        app.update();
        let tick = app
            .world()
            .get_resource_ref::<NativePlayerUiState>()
            .unwrap()
            .last_changed();
        app.update();
        assert_eq!(
            app.world()
                .get_resource_ref::<NativePlayerUiState>()
                .unwrap()
                .last_changed(),
            tick
        );
    }

    use super::*;
    use bevy::input::{
        touch::{TouchInput, TouchPhase},
        InputPlugin,
    };

    #[test]
    fn revive_touch_queues_shared_intent_once_without_local_resurrection() {
        use mir2_ui_core::effect::{GatewayCommand, UiEffect};
        let mut app = App::new();
        let mut state = NativePlayerUiState::default();
        state.core.screen = mir2_ui_core::state::UiScreen::InGame;
        let mut ui = UiReadModel::default();
        ui.player.max_hp = 200;
        ui.player.hp = 0;
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(state)
        .insert_resource(ui)
        .init_resource::<UiEffectQueue>()
        .init_resource::<RailState>()
        .add_systems(Update, buttons);
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Revive,
            BackgroundColor(Color::NONE),
        ));
        app.update();
        app.update(); // Held interaction must not enqueue another request.
        assert_eq!(
            app.world_mut().resource_mut::<UiEffectQueue>().drain(),
            vec![UiEffect::GatewayCommand(GatewayCommand::TownRevive)]
        );
        assert_eq!(app.world().resource::<UiReadModel>().player.hp, 0);
    }

    #[test]
    fn pickup_touch_queues_exact_shared_object_without_local_removal() {
        use mir2_client_bevy::quest_model::RecentPickup;
        let mut pickups = GroundPickupModel::default();
        pickups.upsert(RecentPickup {
            object_id: Some(44),
            key: "object:44".into(),
            label: "Red Potion".into(),
            amount: 2,
            from_npc: Some("Deer".into()),
        });
        let mut state = NativePlayerUiState::default();
        state.core.screen = mir2_ui_core::state::UiScreen::InGame;
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(state)
        .insert_resource(pickups)
        .init_resource::<UiEffectQueue>()
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<RailState>()
        .add_systems(Update, buttons);
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Pickup,
            BackgroundColor(Color::NONE),
        ));
        app.update();
        app.update(); // A held Bevy interaction cannot enqueue twice.
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::PickUpObject { object_id: 44 }]
        );
        let pickups = app.world().resource::<GroundPickupModel>();
        assert_eq!(pickups.recent.len(), 1);
        assert_eq!(pickups.recent[0].object_id, Some(44));
    }

    #[test]
    fn action_pad_attacks_only_the_authoritative_target_and_toggles_run_lock() {
        use mir2_client_bevy::quest_model::CombatTargetUpdate;
        let mut state = NativePlayerUiState::default();
        state.core.screen = mir2_ui_core::state::UiScreen::InGame;
        let mut target = CombatTargetModel::default();
        target.apply(CombatTargetUpdate {
            object_id: 731,
            name: "Hen".into(),
            hp: 9,
            max_hp: 9,
            is_player: false,
        });
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(state)
        .insert_resource(target)
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<RailState>()
        .init_resource::<JoystickState>()
        .add_systems(Update, buttons);
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Attack,
            BackgroundColor(Color::NONE),
        ));
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::RunToggle,
            BackgroundColor(Color::NONE),
        ));
        app.update();
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AttackTarget { object_id: 731 }]
        );
        assert!(!app.world().resource::<JoystickState>().run_lock);
    }

    #[test]
    fn revive_requires_known_dead_player_and_both_active_screens() {
        let mut shell = NativeShellModel::default();
        let mut state = NativePlayerUiState::default();
        let mut ui = UiReadModel::default();
        assert!(!can_request_revive(&shell, &state, Some(&ui)));
        shell.screen = NativeShellScreen::InGame;
        state.core.screen = mir2_ui_core::state::UiScreen::InGame;
        assert!(!can_request_revive(&shell, &state, None));
        ui.player.max_hp = 0;
        assert!(!can_request_revive(&shell, &state, Some(&ui)));
        ui.player.max_hp = 200;
        ui.player.hp = 1;
        assert!(!can_request_revive(&shell, &state, Some(&ui)));
        ui.player.hp = 0;
        assert!(can_request_revive(&shell, &state, Some(&ui)));
        state.core.screen = mir2_ui_core::state::UiScreen::Login;
        assert!(!can_request_revive(&shell, &state, Some(&ui)));
    }

    #[test]
    fn touch_close_also_closes_shared_help() {
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .init_resource::<NativePlayerUiState>()
        .init_resource::<RailState>()
        .add_systems(Update, buttons);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .help
            .open = true;
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Close,
            BackgroundColor(Color::NONE),
        ));
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().help.open);
    }

    #[test]
    fn touch_close_cannot_bypass_amount_modal_or_inactive_shell() {
        use mir2_client_bevy::inventory::{InventoryModel, ItemModel};
        for in_game in [false, true] {
            let mut app = App::new();
            app.insert_resource(NativeShellModel {
                screen: if in_game {
                    NativeShellScreen::InGame
                } else {
                    NativeShellScreen::Login
                },
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .init_resource::<RailState>()
            .add_systems(Update, buttons);
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.help.open = true;
            if in_game {
                let inventory = InventoryModel {
                    items: vec![ItemModel {
                        unique_id: Some(1),
                        quantity: 2,
                        container: 0,
                        slot: 0,
                        ..default()
                    }],
                    ..default()
                };
                assert!(state.open_inventory_delete_for_slot(&inventory, 0));
            }
            drop(state);
            app.world_mut().spawn((
                Interaction::Pressed,
                Action::Close,
                BackgroundColor(Color::NONE),
            ));
            app.update();
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(state.help.open);
            assert_eq!(state.amount_modal_open(), in_game);
        }
    }

    fn pointer_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .init_resource::<TouchPointer>()
            .add_systems(PreUpdate, touch_pointer.after(bevy::input::InputSystems));
        let window = app.world_mut().spawn(Window::default()).id();
        (app, window)
    }

    fn touch(app: &mut App, window: Entity, id: u64, phase: TouchPhase, x: f32) {
        touch_at(app, window, id, phase, Vec2::new(x, 100.0));
    }

    fn touch_at(app: &mut App, window: Entity, id: u64, phase: TouchPhase, position: Vec2) {
        app.world_mut().write_message(TouchInput {
            window,
            id,
            phase,
            position,
            force: None,
        });
    }

    fn joystick_app() -> (App, Entity) {
        let mut app = App::new();
        let mut ui = mir2_ui_core::state::UiState::default();
        ui.screen = mir2_ui_core::state::UiScreen::InGame;
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .insert_resource(ui)
            .init_resource::<AndroidShellState>()
            .init_resource::<JoystickState>()
            .init_resource::<TouchPointer>()
            .init_resource::<crate::android_input::AndroidUiActionQueue>()
            .init_resource::<crate::android_input::AndroidMotionQueue>()
            .add_message::<AndroidInputMessage>()
            .add_systems(
                PreUpdate,
                (joystick_touch, touch_pointer)
                    .chain()
                    .after(bevy::input::InputSystems),
            )
            .add_systems(Update, crate::android_input::route_android_input_messages);
        let window = app.world_mut().spawn(Window::default()).id();
        (app, window)
    }

    #[test]
    fn joystick_claims_a_same_frame_touch_without_a_ghost_ui_click() {
        let (mut app, window) = joystick_app();
        let center = joystick_center(app.world().get::<Window>(window).unwrap(), 0.0, 0.0);
        let position = center + Vec2::new(30.0, -30.0);
        touch_at(&mut app, window, 7, TouchPhase::Started, position);
        touch_at(&mut app, window, 7, TouchPhase::Ended, position);
        app.update();

        let mouse = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(!mouse.just_pressed(MouseButton::Left));
        assert!(!mouse.pressed(MouseButton::Left));
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            None
        );
        assert_eq!(
            app.world()
                .resource::<crate::android_input::AndroidMotionQueue>()
                .0,
            vec![crate::android_input::AndroidMotionIntent {
                direction: crate::android_input::AndroidDirection::UpRight,
                mode: crate::android_input::AndroidMoveMode::Run,
            }]
        );
    }

    #[test]
    fn joystick_owner_does_not_block_a_secondary_action_finger() {
        let (mut app, window) = joystick_app();
        let center = joystick_center(app.world().get::<Window>(window).unwrap(), 0.0, 0.0);
        touch_at(&mut app, window, 1, TouchPhase::Started, center);
        app.update();
        touch_at(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(700.0, 120.0),
        );
        app.update();

        assert_eq!(app.world().resource::<JoystickState>().owner, Some(1));
        assert_eq!(app.world().resource::<TouchPointer>().owner, Some(2));
        assert!(app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left));
    }

    #[test]
    fn ui_hit_testing_observes_the_new_tap_not_the_previous_cursor() {
        #[derive(Resource, Default)]
        struct HitPosition(Option<Vec2>);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .init_resource::<NativeShellModel>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<UiScale>()
            .init_resource::<HitPosition>()
            .add_systems(
                PreUpdate,
                (|windows: Query<&Window>, mut hit: ResMut<HitPosition>| {
                    hit.0 = windows.single().unwrap().cursor_position();
                })
                .in_set(bevy::ui::UiSystems::Focus)
                .after(bevy::input::InputSystems),
            );
        install(&mut app);
        let mut initial = Window::default();
        initial.set_cursor_position(Some(Vec2::new(20.0, 20.0)));
        let window = app.world_mut().spawn(initial).id();
        touch(&mut app, window, 1, TouchPhase::Started, 300.0);
        touch(&mut app, window, 1, TouchPhase::Ended, 300.0);
        app.update();
        assert_eq!(
            app.world().resource::<HitPosition>().0,
            Some(Vec2::new(300.0, 100.0))
        );
    }

    #[test]
    fn same_frame_tap_has_release_edge_and_does_not_hold_mouse() {
        let (mut app, window) = pointer_app();
        touch(&mut app, window, 1, TouchPhase::Started, 300.0);
        touch(&mut app, window, 1, TouchPhase::Ended, 300.0);
        app.update();
        let mouse = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(mouse.just_pressed(MouseButton::Left));
        assert!(mouse.just_released(MouseButton::Left));
        assert!(!mouse.pressed(MouseButton::Left));
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            Some(Vec2::new(300.0, 100.0))
        );
    }

    #[test]
    fn secondary_finger_cannot_inherit_drag_after_owner_releases() {
        let (mut app, window) = pointer_app();
        touch(&mut app, window, 1, TouchPhase::Started, 300.0);
        app.update();
        touch(&mut app, window, 2, TouchPhase::Started, 600.0);
        touch(&mut app, window, 1, TouchPhase::Ended, 300.0);
        app.update();
        assert!(!app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left));
        app.update();
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            Some(Vec2::new(300.0, 100.0))
        );
    }

    #[test]
    fn android_drag_handles_keep_system_gesture_gutter() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(UiScale(0.5))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .add_systems(Update, keep_drag_handles_reachable);
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .inventory_window
                .top,
            48.0
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .inventory_window
            .top = 200.0;
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .inventory_window
                .top,
            200.0
        );
    }

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
        let mut query = app.world_mut().query::<(
            &Action,
            &Node,
            Option<&RailButton>,
            Option<&ActionPadButton>,
        )>();
        let mut visible_rail = 0;
        let mut visible_pad = 0;
        for (_, node, rail, pad) in query.iter(app.world()) {
            if rail.is_some() {
                assert_eq!(node.height, px(96));
            } else if pad.is_some() {
                assert_eq!(node.height, px(112));
                assert_eq!(node.width, px(136));
            }
            if node.display != Display::None {
                visible_rail += usize::from(rail.is_some());
                visible_pad += usize::from(pad.is_some());
            }
        }
        assert_eq!(visible_rail, 1, "rail is collapsed by default");
        assert_eq!(visible_pad, 4, "combat pad stays directly reachable");
        let mut ui = UiReadModel::default();
        ui.player.hp = 0;
        ui.player.max_hp = 200;
        app.insert_resource(ui);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .screen = mir2_ui_core::state::UiScreen::InGame;
        app.update();
        let world = app.world_mut();
        let mut targets = world.query::<(&Action, &Node)>();
        let revive = targets
            .iter(world)
            .find(|(action, _)| matches!(action, Action::Revive))
            .unwrap()
            .1;
        assert_eq!(revive.display, Display::Flex);
        assert_eq!(revive.height, px(96));
        assert_eq!(revive.width, px(128));
    }
}
