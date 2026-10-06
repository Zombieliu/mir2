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

use crate::android_input::{
    AndroidInputEvent, AndroidInputMessage, AndroidLifecycle, AndroidShellState,
};

const JOYSTICK_DIAMETER: f32 = 144.0;
const JOYSTICK_KNOB_DIAMETER: f32 = 56.0;
const JOYSTICK_TRAVEL: f32 = 42.0;
const JOYSTICK_LEFT: f32 = 24.0;
const JOYSTICK_BOTTOM: f32 = 20.0;
const JOYSTICK_EMIT_SECONDS: f64 = 0.1;
const CONTROL_SHORT_EDGE_FRACTION: f32 = 0.16;
const JOYSTICK_MIN_DIAMETER: f32 = 48.0;
const CONTROL_MIN_BUTTON_HEIGHT: f32 = 48.0;
const COMPACT_ACTION_PAD_MAX_HEIGHT: f32 = 320.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct GameplayControlMetrics {
    joystick_scale: f32,
    rail_width: f32,
    rail_button_width: f32,
    action_pad_width: f32,
    button_width: f32,
    button_height: f32,
    gap: f32,
}

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
struct NpcServiceCloseTarget;
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
    context: Option<TouchContext>,
    native_cursor_frame: NativeTouchCursorFrame,
}

#[derive(Default)]
struct NativeTouchCursorFrame {
    previous: Option<(Entity, Option<Vec2>)>,
}

impl NativeTouchCursorFrame {
    fn capture(&mut self, entity: Entity, window: &Window) {
        if self.previous.is_none() {
            self.previous = Some((entity, window.physical_cursor_position()));
        }
    }
}

#[derive(Resource, Default)]
struct SecondaryTouchInteractions {
    pressed: Vec<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TouchContext {
    shell_screen: NativeShellScreen,
    rail_expanded: bool,
    panel: mir2_ui_core::state::UiPanel,
    security_panel: mir2_ui_core::state::UiSecurityPanel,
    chat_focused: bool,
    amount_modal_open: bool,
    keyboard_open: bool,
    help_open: bool,
    trade_dialog_open: bool,
    lifecycle: Option<AndroidLifecycle>,
    window_focused: bool,
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
    #[cfg(target_os = "android")]
    install_android_touch_cursor_frame(app);
    app.init_resource::<RailState>()
        .init_resource::<UiEffectQueue>()
        .init_resource::<TouchPointer>()
        .init_resource::<SecondaryTouchInteractions>()
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
            Update,
            npc_service_close_buttons
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
            PreUpdate,
            secondary_touch_buttons.after(bevy::ui::UiSystems::Focus),
        )
        .add_systems(
            PostUpdate,
            (visibility, joystick_visual)
                .chain()
                .after(super::shared_shell::AndroidStageFit)
                .before(bevy::ui::UiSystems::Layout),
        )
        .add_systems(
            PostUpdate,
            sync_npc_service_close_target
                .after(super::shared_shell::AndroidStageFit)
                .before(bevy::ui::UiSystems::Layout),
        );
}

fn spawn_npc_service_close_target(commands: &mut Commands, node: Node) -> Entity {
    // NPCDrop's red X is baked into Prguse2/351, not a shared Button. Add a
    // phone-sized transparent target over that same art, not another NPC rule.
    commands
        .spawn((
            Name::new("AndroidNpcServiceCloseTarget"),
            NpcServiceCloseTarget,
            Button,
            node,
            BackgroundColor(Color::NONE),
            ZIndex(50),
        ))
        .id()
}

fn npc_service_close_node(
    shell: &NativeShellModel,
    player: &NativePlayerUiState,
    shop: &mir2_client_bevy::shop::ShopModel,
    scale: f32,
    transform: &UiTransform,
) -> Option<Node> {
    if shell.screen != NativeShellScreen::InGame
        || player.core.screen != mir2_ui_core::state::UiScreen::InGame
        || !player.npc_shop_open()
        || player.amount_modal_open()
        || player.keyboard.open
        || player.trade_dialog.open
    {
        return None;
    }
    let rendered_scale = transform.scale.abs() * scale;
    if !rendered_scale.is_finite() || rendered_scale.min_element() <= 0.0 {
        return None;
    }
    let buy = shop.allows_buy() && (!shop.allows_sell() || player.npc_shop_buy_tab);
    let (center, controls_top) = if buy {
        // Shared NPCGoods close at (217,3),24x21; arrows start at y35.
        (Vec2::new(229.0, 13.5), 33.0)
    } else if shop.allows_sell() || shop.allows_repair() || shop.allows_special_repair() {
        // NPCDrop is source-positioned at x264; its baked X is (161,12).
        // Keep the larger target above the Hold control which starts at y36.
        (Vec2::new(264.0 + 161.0, 12.0), 34.0)
    } else {
        return None;
    };
    let size = Vec2::splat(48.0) / rendered_scale;
    Some(Node {
        position_type: PositionType::Absolute,
        left: px(center.x - size.x * 0.5),
        top: px((center.y - size.y * 0.5).min(controls_top - size.y)),
        width: px(size.x),
        height: px(size.y),
        ..default()
    })
}

fn sync_npc_service_close_target(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    player: Res<NativePlayerUiState>,
    shop: Option<Res<mir2_client_bevy::shop::ShopModel>>,
    scale: Res<UiScale>,
    roots: Query<
        (Entity, &Node, &UiTransform),
        (
            With<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
            Without<NpcServiceCloseTarget>,
        ),
    >,
    mut targets: Query<(Entity, &mut Node, Option<&ChildOf>), With<NpcServiceCloseTarget>>,
) {
    let root = roots.single().ok();
    let active_node = root.and_then(|(_, node, transform)| {
        (node.display != Display::None)
            .then(|| shop.as_deref())
            .flatten()
            .and_then(|shop| npc_service_close_node(&shell, &player, shop, scale.0, transform))
    });
    if let Ok((entity, mut node, parent)) = targets.single_mut() {
        if let Some(active) = active_node {
            if *node != active {
                *node = active;
            }
            if let Some((root, _, _)) = root {
                if parent.map(ChildOf::parent) != Some(root) {
                    commands.entity(entity).insert(ChildOf(root));
                }
            }
        } else {
            node.display = Display::None;
        }
    } else if targets.is_empty() {
        if let (Some(node), Some((root, _, _))) = (active_node, root) {
            // Shared rendering may replace the window's children. Recreate
            // only this platform target after that rebuild, before UI layout.
            let entity = spawn_npc_service_close_target(&mut commands, node);
            commands.entity(entity).insert(ChildOf(root));
        }
    }
}

fn npc_service_close_buttons(
    shell: Res<NativeShellModel>,
    mut player: ResMut<NativePlayerUiState>,
    roots: Query<
        (Entity, &Node, Option<&InheritedVisibility>),
        With<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
    >,
    targets: Query<
        (&Interaction, &Node, &ChildOf, Option<&InheritedVisibility>),
        (With<NpcServiceCloseTarget>, Changed<Interaction>),
    >,
) {
    if shell.screen != NativeShellScreen::InGame
        || player.core.screen != mir2_ui_core::state::UiScreen::InGame
        || !player.npc_shop_open()
        || player.amount_modal_open()
        || player.keyboard.open
        || player.trade_dialog.open
    {
        return;
    }
    let Ok((root, root_node, root_visibility)) = roots.single() else {
        return;
    };
    if root_node.display == Display::None || root_visibility.is_some_and(|visible| !visible.get()) {
        return;
    }
    if targets
        .iter()
        .any(|(interaction, node, parent, visibility)| {
            *interaction == Interaction::Pressed
                && node.display != Display::None
                && parent.parent() == root
                && visibility.is_none_or(|visible| visible.get())
        })
    {
        // Existing shared lifecycle cancels the request and owns the Exit
        // intent. No local NPC transaction, quote, currency or save mutation.
        player.close_all_windows();
        #[cfg(feature = "ui-preview")]
        info!("ANDROID_NPC_CLOSE_SHARED_LOCAL_UI_EXIT_REQUEST_NOT_LIVE");
    }
}

// Bevy UI already handles touch clicks. Source Crystal drag/scroll handlers
// additionally read Window cursor + left-button state, so bridge one owning
// finger to those SAME handlers rather than implementing new item operations.
fn joystick_touch(
    touches: Option<Res<Touches>>,
    time: Option<Res<Time>>,
    shell: Res<NativeShellModel>,
    state: Res<NativePlayerUiState>,
    world_input: crate::world_input::WorldInputContext,
    rail: Option<Res<RailState>>,
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
    let blocked =
        world_input.blocks_actions(&state) || rail.as_deref().is_some_and(|rail| rail.expanded);
    if shell.screen != NativeShellScreen::InGame
        || blocked
        || !window.focused
        || android
            .as_deref()
            .is_some_and(|state| state.lifecycle != AndroidLifecycle::Foreground)
    {
        #[cfg(feature = "ui-preview")]
        if let Some(owner) = joystick.owner {
            info!(
                owner,
                screen = ?shell.screen,
                blocked,
                focused = window.focused,
                lifecycle = ?android.as_deref().map(|state| state.lifecycle),
                "ANDROID_UI_PREVIEW_TOUCH_CANCEL joystick"
            );
        }
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
    let metrics = gameplay_control_metrics(window.height());
    let center = joystick_center(window, safe_left, safe_bottom, metrics.joystick_scale);
    if joystick.owner.is_none() {
        joystick.owner = touches
            .iter_just_pressed()
            .filter(|touch| {
                touch.position().distance(center)
                    <= JOYSTICK_DIAMETER * metrics.joystick_scale * 0.62
            })
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
    joystick.vector =
        ((position - center) / (JOYSTICK_TRAVEL * metrics.joystick_scale)).clamp_length_max(1.0);
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
            #[cfg(feature = "ui-preview")]
            info!(
                x = joystick.vector.x,
                y = joystick.vector.y,
                run = joystick.run_lock,
                "ANDROID_UI_PREVIEW_INTENT move (queued only, no server)"
            );
        }
        joystick.last_emit_at = Some(now);
    }
    if released {
        joystick.release();
        joystick.claimed_this_frame.push(owner);
    }
}

fn gameplay_control_metrics(viewport_height: f32) -> GameplayControlMetrics {
    let joystick_diameter = (viewport_height * CONTROL_SHORT_EDGE_FRACTION)
        .clamp(JOYSTICK_MIN_DIAMETER, JOYSTICK_DIAMETER);
    if viewport_height < COMPACT_ACTION_PAD_MAX_HEIGHT {
        let button_width = 48.0;
        let gap = 6.0;
        GameplayControlMetrics {
            joystick_scale: joystick_diameter / JOYSTICK_DIAMETER,
            rail_width: 204.0,
            rail_button_width: 48.0,
            action_pad_width: button_width * 4.0 + gap * 3.0,
            button_width,
            button_height: CONTROL_MIN_BUTTON_HEIGHT,
            gap,
        }
    } else {
        let button_scale = (viewport_height / 480.0).clamp(0.8, 1.0);
        let button_width = 68.0 * button_scale;
        let gap = 8.0 * button_scale;
        GameplayControlMetrics {
            joystick_scale: joystick_diameter / JOYSTICK_DIAMETER,
            rail_width: 132.0,
            rail_button_width: 64.0,
            action_pad_width: button_width * 2.0 + gap,
            button_width,
            button_height: (56.0 * button_scale).max(CONTROL_MIN_BUTTON_HEIGHT),
            gap,
        }
    }
}

/// Left/right thumb footprints used by the phone chat layout. Keep this tied
/// to the same metrics used for rendering and touch ownership, including the
/// four-button compact row on short landscape screens.
pub(crate) fn thumb_footprints(viewport_height: f32) -> Vec2 {
    let metrics = gameplay_control_metrics(viewport_height);
    Vec2::new(
        JOYSTICK_LEFT + JOYSTICK_DIAMETER * metrics.joystick_scale + 16.0,
        20.0 + metrics.action_pad_width + 16.0,
    )
}

fn joystick_center(window: &Window, safe_left: f32, safe_bottom: f32, control_scale: f32) -> Vec2 {
    let diameter = JOYSTICK_DIAMETER * control_scale;
    Vec2::new(
        safe_left + JOYSTICK_LEFT + diameter * 0.5,
        window.height() - safe_bottom - JOYSTICK_BOTTOM - diameter * 0.5,
    )
}

fn touch_pointer(
    touches: Option<Res<Touches>>,
    joystick: Option<Res<JoystickState>>,
    shell: Option<Res<NativeShellModel>>,
    state: Option<Res<NativePlayerUiState>>,
    rail: Option<Res<RailState>>,
    android: Option<Res<AndroidShellState>>,
    mut pointer: ResMut<TouchPointer>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: Option<ResMut<ButtonInput<MouseButton>>>,
    mut cursor_moves: Option<MessageWriter<bevy::window::CursorMoved>>,
) {
    let (Some(touches), Some(mouse), Ok((window_entity, mut window))) =
        (touches, mouse.as_deref_mut(), windows.single_mut())
    else {
        return;
    };
    #[cfg(target_os = "android")]
    pointer.native_cursor_frame.capture(window_entity, &window);
    let context = TouchContext {
        rail_expanded: rail.as_deref().is_some_and(|rail| rail.expanded),
        shell_screen: shell
            .as_deref()
            .map(|shell| shell.screen)
            .unwrap_or_default(),
        panel: state
            .as_deref()
            .map(|state| state.core.panel)
            .unwrap_or_default(),
        security_panel: state
            .as_deref()
            .map(|state| state.core.security.panel)
            .unwrap_or_default(),
        chat_focused: state.as_deref().is_some_and(|state| state.chat_focused()),
        amount_modal_open: state
            .as_deref()
            .is_some_and(|state| state.amount_modal_open()),
        keyboard_open: state.as_deref().is_some_and(|state| state.keyboard.open),
        help_open: state.as_deref().is_some_and(|state| state.help_open()),
        trade_dialog_open: state
            .as_deref()
            .is_some_and(|state| state.trade_dialog.open),
        lifecycle: android.as_deref().map(|state| state.lifecycle),
        window_focused: window.focused,
    };
    let context_changed = pointer
        .context
        .replace(context)
        .is_some_and(|previous| previous != context);
    let inactive = !window.focused
        || context
            .lifecycle
            .is_some_and(|lifecycle| lifecycle != AndroidLifecycle::Foreground);
    if inactive || context_changed {
        #[cfg(feature = "ui-preview")]
        if pointer.owner.is_some() || mouse.pressed(MouseButton::Left) {
            info!(
                owner = ?pointer.owner,
                inactive,
                context_changed,
                "ANDROID_UI_PREVIEW_TOUCH_CANCEL pointer"
            );
        }
        if pointer.owner.take().is_some() || mouse.pressed(MouseButton::Left) {
            mouse.release(MouseButton::Left);
        }
        pointer.wait_for_release = true;
        clear_touch_cursor(&mut window);
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
            publish_touch_cursor(
                window_entity,
                &mut window,
                touch.position(),
                &mut cursor_moves,
            );
        } else {
            if let Some(touch) = touches.get_released(owner) {
                publish_touch_cursor(
                    window_entity,
                    &mut window,
                    touch.position(),
                    &mut cursor_moves,
                );
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
        publish_touch_cursor(
            window_entity,
            &mut window,
            touch.position(),
            &mut cursor_moves,
        );
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

// Bevy 0.19 UI resolves touch hover from `first_pressed_position`, so the
// joystick owner otherwise masks every later finger.  Activate only Android's
// own action buttons here; shared Crystal windows continue through the single
// pointer bridge above so their drag/click ownership remains serialized.
fn secondary_touch_buttons(
    touches: Option<Res<Touches>>,
    joystick: Res<JoystickState>,
    windows: Query<&Window>,
    mut state: ResMut<SecondaryTouchInteractions>,
    mut buttons: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&InheritedVisibility>,
            &mut Interaction,
        ),
        (With<Action>, Or<(With<ActionPadButton>, With<RailButton>)>),
    >,
) {
    for entity in std::mem::take(&mut state.pressed) {
        if let Ok((_, _, _, _, mut interaction)) = buttons.get_mut(entity) {
            interaction.set_if_neq(Interaction::None);
        }
    }
    let (Some(touches), Some(_), Ok(window)) = (touches, joystick.owner, windows.single()) else {
        return;
    };
    let scale_factor = window.scale_factor();
    for touch in touches
        .iter_just_pressed()
        .filter(|touch| !joystick.claims(touch.id()))
    {
        let point = touch.position() * scale_factor;
        for (entity, node, transform, visibility, mut interaction) in &mut buttons {
            if visibility.is_some_and(|visibility| !visibility.get())
                || !node.contains_point(*transform, point)
            {
                continue;
            }
            interaction.set_if_neq(Interaction::Pressed);
            state.pressed.push(entity);
            break;
        }
    }
}

fn publish_touch_cursor(
    window_entity: Entity,
    window: &mut Window,
    position: Vec2,
    cursor_moves: &mut Option<MessageWriter<bevy::window::CursorMoved>>,
) {
    #[cfg(target_os = "android")]
    {
        publish_android_touch_cursor(window_entity, window, position, cursor_moves);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (window_entity, cursor_moves);
        window.set_cursor_position(Some(position));
    }
}

fn clear_touch_cursor(window: &mut Window) {
    #[cfg(target_os = "android")]
    clear_android_touch_cursor(window);
    #[cfg(not(target_os = "android"))]
    window.set_cursor_position(None);
}

// Keep the actual Android branch callable by host regression tests; testing
// the desktop setter does not establish the mobile shared-window contract.
fn publish_android_touch_cursor(
    window_entity: Entity,
    window: &mut Window,
    position: Vec2,
    cursor_moves: &mut Option<MessageWriter<bevy::window::CursorMoved>>,
) {
    // Shared Crystal handlers read Window.cursor_position, not CursorMoved.
    // This snapshot is visible to same-frame UI input only; restoring it in
    // PostUpdate prevents Winit's Last stage from warping an Android OS cursor.
    window.set_cursor_position(Some(position));
    if let Some(cursor_moves) = cursor_moves.as_mut() {
        cursor_moves.write(bevy::window::CursorMoved {
            window: window_entity,
            position,
            delta: None,
        });
    }
}

fn clear_android_touch_cursor(window: &mut Window) {
    window.set_cursor_position(None);
}

fn install_android_touch_cursor_frame(app: &mut App) {
    app.add_systems(
        PostUpdate,
        restore_android_touch_cursor_frame.after(bevy::ui::UiSystems::Layout),
    );
}

fn restore_android_touch_cursor_frame(
    mut pointer: ResMut<TouchPointer>,
    mut windows: Query<&mut Window>,
) {
    let Some((entity, previous)) = pointer.native_cursor_frame.previous.take() else { return; };
    if let Ok(mut window) = windows.get_mut(entity) {
        window.set_physical_cursor_position(previous.map(|position| position.as_dvec2()));
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

#[cfg(test)]
#[path = "touch_cursor_bridge_tests.rs"]
mod touch_cursor_bridge_tests;
fn spawn(mut commands: Commands) {
    spawn_npc_service_close_target(
        &mut commands,
        Node {
            position_type: PositionType::Absolute,
            width: px(48.0),
            height: px(48.0),
            display: Display::None,
            ..default()
        },
    );
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
                        mir2_client_bevy::crystal_ui::typography::crystal_text_font(16.0),
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
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.72)),
                    BorderColor::all(Color::srgb(0.70, 0.53, 0.24)),
                ))
                .with_children(|button| {
                    button.spawn((
                        RailLabel,
                        Text::new(label),
                        mir2_client_bevy::crystal_ui::typography::crystal_text_font(15.0),
                        TextColor(Color::srgb(0.98, 0.88, 0.64)),
                    ));
                });
            }
        });
}
fn visibility(
    shell: Res<NativeShellModel>,
    state: Res<NativePlayerUiState>,
    world_input: crate::world_input::WorldInputContext,
    map: Option<Res<mir2_client_bevy::map::MapModel>>,
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
    let ui = world_input.read_model();
    let unit = 1.0 / scale.0.max(0.01);
    let metrics = windows
        .single()
        .map(|window| gameplay_control_metrics(window.height()))
        .unwrap_or_else(|_| gameplay_control_metrics(1_080.0));
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
        map.as_ref().and_then(|map| map.mini_map_index),
        map.as_ref().and_then(|map| map.map_width),
        map.as_ref().and_then(|map| map.map_height),
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
    // Death still exposes the existing Revive action; movement is rejected by
    // joystick_touch and the authoritative shared action path separately.
    let world_controls_visible = in_game && !world_input.blocks_views(&state) && !rail.expanded;
    for mut node in &mut rail_roots {
        node.width = px(if rail.expanded {
            metrics.rail_width
        } else {
            metrics.rail_button_width
        } * unit);
        node.top = px((safe_top + 16.0 + map_bottom * scale.0).max(48.0) * unit);
        node.right = px((safe_right + 8.0) * unit);
        node.row_gap = px(4.0 * unit);
        node.column_gap = px(4.0 * unit);
        node.display = if in_game && rail.expanded {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (action, mut node) in &mut rail_buttons {
        node.width = px(metrics.rail_button_width * unit);
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
        node.width = px(metrics.action_pad_width * unit);
        node.right = px((safe_right + 20.0) * unit);
        node.bottom = px((safe_bottom + JOYSTICK_BOTTOM) * unit);
        node.row_gap = px(metrics.gap * unit);
        node.column_gap = px(metrics.gap * unit);
        node.display = if world_controls_visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut pad_buttons {
        node.width = px(metrics.button_width * unit);
        node.height = px(metrics.button_height * unit);
        node.display = Display::Flex;
    }
    for mut node in &mut joystick_roots {
        node.width = px(JOYSTICK_DIAMETER * metrics.joystick_scale * unit);
        node.height = px(JOYSTICK_DIAMETER * metrics.joystick_scale * unit);
        node.left = px((safe_left + JOYSTICK_LEFT) * unit);
        node.bottom = px((safe_bottom + JOYSTICK_BOTTOM) * unit);
        node.border = UiRect::all(px(2.0 * unit));
        node.display = if world_controls_visible {
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
    windows: Query<&Window>,
    mut knobs: Query<&mut Node, With<JoystickKnob>>,
) {
    let unit = 1.0 / scale.0.max(0.01);
    let control_scale = windows
        .single()
        .map(|window| gameplay_control_metrics(window.height()).joystick_scale)
        .unwrap_or(1.0);
    let offset = joystick.vector * JOYSTICK_TRAVEL * control_scale;
    let centered = (JOYSTICK_DIAMETER - JOYSTICK_KNOB_DIAMETER) * control_scale * 0.5;
    for mut node in &mut knobs {
        node.width = px(JOYSTICK_KNOB_DIAMETER * control_scale * unit);
        node.height = px(JOYSTICK_KNOB_DIAMETER * control_scale * unit);
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
            Color::srgba(0.25, 0.19, 0.08, 0.80)
        } else {
            Color::srgba(0.04, 0.05, 0.06, 0.72)
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
                        Color::srgba(0.25, 0.19, 0.08, 0.80)
                    } else {
                        Color::srgba(0.04, 0.05, 0.06, 0.72)
                    };
                    #[cfg(feature = "ui-preview")]
                    info!(
                        run = joystick.run_lock,
                        "ANDROID_UI_PREVIEW_INTENT runMode (local control only)"
                    );
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
            Action::Skills => {
                state.toggle_skill();
                #[cfg(feature = "ui-preview")]
                info!("ANDROID_UI_PREVIEW_ACTION skills (local panel only)");
            }
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
    fn secondary_actions_keep_exact_targets_while_the_joystick_is_owned() {
        use mir2_client_bevy::quest_model::CombatTargetUpdate;
        use mir2_client_bevy::quest_model::RecentPickup;
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
        let mut pickups = GroundPickupModel::default();
        pickups.upsert(RecentPickup {
            object_id: Some(44),
            key: "object:44".into(),
            label: "Red Potion".into(),
            amount: 2,
            from_npc: Some("Hen".into()),
        });
        let joystick = JoystickState {
            owner: Some(17),
            ..default()
        };
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(state)
        .insert_resource(target)
        .insert_resource(pickups)
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<RailState>()
        .insert_resource(joystick)
        .add_systems(Update, buttons);
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Attack,
            BackgroundColor(Color::NONE),
        ));
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Pickup,
            BackgroundColor(Color::NONE),
        ));
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::RunToggle,
            BackgroundColor(Color::NONE),
        ));
        app.world_mut().spawn((
            Interaction::Pressed,
            Action::Skills,
            BackgroundColor(Color::NONE),
        ));
        app.update();
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![
                QuestUiIntent::AttackTarget { object_id: 731 },
                QuestUiIntent::PickUpObject { object_id: 44 },
            ]
        );
        let joystick = app.world().resource::<JoystickState>();
        assert_eq!(joystick.owner, Some(17));
        assert!(!joystick.run_lock);
        assert!(app.world().resource::<NativePlayerUiState>().skill_open());
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
            .init_resource::<NativePlayerUiState>()
            .insert_resource(ui)
            .insert_resource(AndroidShellState {
                lifecycle: AndroidLifecycle::Foreground,
                ..default()
            })
            .init_resource::<JoystickState>()
            .init_resource::<TouchPointer>()
            .init_resource::<SecondaryTouchInteractions>()
            .init_resource::<crate::android_input::AndroidUiActionQueue>()
            .init_resource::<crate::android_input::AndroidMotionQueue>()
            .add_message::<AndroidInputMessage>()
            .add_systems(
                PreUpdate,
                (joystick_touch, touch_pointer, secondary_touch_buttons)
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
        let window_ref = app.world().get::<Window>(window).unwrap();
        let center = joystick_center(
            window_ref,
            0.0,
            0.0,
            gameplay_control_metrics(window_ref.height()).joystick_scale,
        );
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
    fn commerce_panel_disables_the_world_joystick_gesture() {
        let (mut app, window) = joystick_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::NpcShop;
        let window_ref = app.world().get::<Window>(window).unwrap();
        let center = joystick_center(
            window_ref,
            0.0,
            0.0,
            gameplay_control_metrics(window_ref.height()).joystick_scale,
        );
        touch_at(&mut app, window, 7, TouchPhase::Started, center);
        app.update();

        assert_eq!(app.world().resource::<JoystickState>().owner, None);
        assert!(app
            .world()
            .resource::<crate::android_input::AndroidMotionQueue>()
            .0
            .is_empty());
    }

    #[test]
    fn rail_transition_releases_the_old_pointer_without_a_ghost_menu_click() {
        let (mut app, window) = joystick_app();
        app.init_resource::<RailState>();
        touch_at(
            &mut app,
            window,
            11,
            TouchPhase::Started,
            Vec2::new(700.0, 120.0),
        );
        app.update();
        assert_eq!(app.world().resource::<TouchPointer>().owner, Some(11));
        assert!(app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left));
        app.world_mut().resource_mut::<RailState>().expanded = true;
        app.update();
        assert_eq!(app.world().resource::<TouchPointer>().owner, None);
        assert!(app.world().resource::<TouchPointer>().wait_for_release);
        assert!(!app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left));
        app.update();
        assert_eq!(app.world().resource::<TouchPointer>().owner, None);
        touch_at(
            &mut app,
            window,
            11,
            TouchPhase::Ended,
            Vec2::new(700.0, 120.0),
        );
        app.update();
        app.update();
        assert!(!app.world().resource::<TouchPointer>().wait_for_release);
        assert!(!app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .just_pressed(MouseButton::Left));
    }

    #[test]
    fn expanded_phone_rail_cancels_owned_and_rejects_fresh_hidden_joystick_motion() {
        for owned in [false, true] {
            let (mut app, window) = joystick_app();
            app.init_resource::<RailState>();
            let window_ref = app.world().get::<Window>(window).unwrap();
            let center = joystick_center(
                window_ref,
                0.0,
                0.0,
                gameplay_control_metrics(window_ref.height()).joystick_scale,
            );
            let position = center + Vec2::new(30.0, 0.0);
            if owned {
                touch_at(&mut app, window, 9, TouchPhase::Started, position);
                app.update();
                assert_eq!(app.world().resource::<JoystickState>().owner, Some(9));
                app.world_mut()
                    .resource_mut::<crate::android_input::AndroidMotionQueue>()
                    .0
                    .clear();
            }
            app.world_mut().resource_mut::<RailState>().expanded = true;
            if !owned {
                touch_at(&mut app, window, 9, TouchPhase::Started, position);
            }
            app.update();
            assert_eq!(
                app.world().resource::<JoystickState>().owner,
                None,
                "owned={owned}"
            );
            assert!(
                app.world()
                    .resource::<crate::android_input::AndroidMotionQueue>()
                    .0
                    .is_empty(),
                "a hidden phone joystick must not queue movement, owned={owned}"
            );
        }
    }

    #[test]
    fn nonmodal_views_keep_the_android_joystick_and_motion_intent_available() {
        use mir2_ui_core::state::UiPanel;
        for panel in [
            UiPanel::Inventory,
            UiPanel::Character,
            UiPanel::Skill,
            UiPanel::Options,
            UiPanel::Menu,
            UiPanel::QuestLog,
        ] {
            let (mut app, window) = joystick_app();
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .panel = panel;
            let window_ref = app.world().get::<Window>(window).unwrap();
            let center = joystick_center(
                window_ref,
                0.0,
                0.0,
                gameplay_control_metrics(window_ref.height()).joystick_scale,
            );
            touch_at(
                &mut app,
                window,
                17,
                TouchPhase::Started,
                center + Vec2::new(30.0, 0.0),
            );
            app.update();
            assert_eq!(
                app.world().resource::<JoystickState>().owner,
                Some(17),
                "{panel:?}"
            );
            assert_eq!(
                app.world()
                    .resource::<crate::android_input::AndroidMotionQueue>()
                    .0
                    .len(),
                1,
                "{panel:?} suppressed ordinary movement"
            );
            assert!(
                !app.world()
                    .resource::<ButtonInput<MouseButton>>()
                    .just_pressed(MouseButton::Left),
                "joystick touch leaked through to the shared panel"
            );
        }
    }

    #[test]
    fn joystick_owner_does_not_block_a_secondary_action_finger() {
        let (mut app, window) = joystick_app();
        let window_ref = app.world().get::<Window>(window).unwrap();
        let center = joystick_center(
            window_ref,
            0.0,
            0.0,
            gameplay_control_metrics(window_ref.height()).joystick_scale,
        );
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
    fn secondary_action_finger_activates_the_button_while_joystick_is_owned() {
        let (mut app, window) = joystick_app();
        let window_ref = app.world().get::<Window>(window).unwrap();
        let center = joystick_center(
            window_ref,
            0.0,
            0.0,
            gameplay_control_metrics(window_ref.height()).joystick_scale,
        );
        let button = app
            .world_mut()
            .spawn((
                ActionPadButton,
                Action::Attack,
                Interaction::None,
                ComputedNode {
                    size: Vec2::splat(80.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from_xy(600.0, 100.0),
            ))
            .id();

        touch_at(&mut app, window, 1, TouchPhase::Started, center);
        app.update();
        touch_at(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(600.0, 100.0),
        );
        app.update();

        assert_eq!(app.world().resource::<JoystickState>().owner, Some(1));
        assert_eq!(
            *app.world().get::<Interaction>(button).unwrap(),
            Interaction::Pressed
        );
        app.update();
        assert_eq!(
            *app.world().get::<Interaction>(button).unwrap(),
            Interaction::None
        );
    }

    #[test]
    fn panel_transition_keeps_nonmodal_motion_but_cancels_modal_and_pointer_owners() {
        use mir2_ui_core::state::UiPanel;
        for (panel, modal) in [(UiPanel::Inventory, false), (UiPanel::NpcShop, true)] {
            let (mut app, window) = joystick_app();
            let window_ref = app.world().get::<Window>(window).unwrap();
            let center = joystick_center(
                window_ref,
                0.0,
                0.0,
                gameplay_control_metrics(window_ref.height()).joystick_scale,
            );
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

            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .panel = panel;
            touch_at(
                &mut app,
                window,
                1,
                TouchPhase::Moved,
                center + Vec2::new(30.0, 0.0),
            );
            app.update();

            assert_eq!(
                app.world().resource::<JoystickState>().owner,
                if modal { None } else { Some(1) },
                "{panel:?}"
            );
            assert_eq!(
                app.world()
                    .resource::<crate::android_input::AndroidMotionQueue>()
                    .0
                    .len(),
                if modal { 0 } else { 1 },
                "{panel:?} must retain ordinary motion but stop service movement"
            );
            // A held panel finger must never become a click in the newly opened
            // view. The dedicated joystick is independent of that pointer.
            let pointer = app.world().resource::<TouchPointer>();
            assert_eq!(pointer.owner, None);
            assert!(pointer.wait_for_release);
            assert!(!app
                .world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left));
        }
    }

    #[test]
    fn true_modals_and_death_block_fresh_and_owned_joystick_over_an_ordinary_view() {
        use mir2_client_bevy::{
            crystal_ui::notice::{NoticeDialogState, NoticePacketUpdate},
            quest_model::NpcDialogModel,
            quest_ui::QuestUiState,
        };
        for owned in [false, true] {
            for guard in [
                "npc",
                "notice",
                "abandon",
                "quest-alert",
                "chat",
                "skill-assign",
                "dead",
            ] {
                let (mut app, window) = joystick_app();
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .core
                    .panel = mir2_ui_core::state::UiPanel::Inventory;
                let window_ref = app.world().get::<Window>(window).unwrap();
                let center = joystick_center(
                    window_ref,
                    0.0,
                    0.0,
                    gameplay_control_metrics(window_ref.height()).joystick_scale,
                );
                let position = center + Vec2::new(30.0, 0.0);
                if owned {
                    touch_at(&mut app, window, 9, TouchPhase::Started, position);
                    app.update();
                    assert_eq!(app.world().resource::<JoystickState>().owner, Some(9));
                    app.world_mut()
                        .resource_mut::<crate::android_input::AndroidMotionQueue>()
                        .0
                        .clear();
                }
                match guard {
                    "npc" => {
                        app.insert_resource(NpcDialogModel {
                            is_open: true,
                            ..default()
                        });
                    }
                    "notice" => {
                        let mut notice = NoticeDialogState::default();
                        assert!(notice.observe(NoticePacketUpdate {
                            generation: 1,
                            sequence: 1,
                            title: "Offline guard fixture".into(),
                            message: "Modal fixture, not a server receipt".into(),
                        }));
                        app.insert_resource(notice);
                    }
                    "abandon" => {
                        app.insert_resource(QuestUiState {
                            abandon_confirmation_quest_index: Some(7),
                            ..default()
                        });
                    }
                    "quest-alert" => {
                        app.insert_resource(QuestUiState {
                            quest_alert_message: Some("Offline guard fixture".into()),
                            ..default()
                        });
                    }
                    "chat" => app
                        .world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .set_chat_focused(true),
                    "skill-assign" => {
                        app.world_mut()
                            .resource_mut::<NativePlayerUiState>()
                            .skill_assign
                            .open = true
                    }
                    "dead" => {
                        let mut model = UiReadModel::default();
                        model.player.max_hp = 30;
                        model.player.hp = 0;
                        app.insert_resource(model);
                    }
                    _ => unreachable!(),
                }
                if !owned {
                    touch_at(&mut app, window, 9, TouchPhase::Started, position);
                }
                app.update();
                let joystick = app.world().resource::<JoystickState>();
                assert_eq!(joystick.owner, None, "{guard}, owned={owned}");
                assert_eq!(joystick.vector, Vec2::ZERO, "{guard}, owned={owned}");
                assert!(
                    app.world()
                        .resource::<crate::android_input::AndroidMotionQueue>()
                        .0
                        .is_empty(),
                    "{guard}, owned={owned} leaked a movement intent"
                );
            }
        }
    }

    #[test]
    fn background_cancels_touch_owners_and_resume_does_not_reclaim_held_fingers() {
        let (mut app, window) = joystick_app();
        let window_ref = app.world().get::<Window>(window).unwrap();
        let center = joystick_center(
            window_ref,
            0.0,
            0.0,
            gameplay_control_metrics(window_ref.height()).joystick_scale,
        );
        touch_at(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            center + Vec2::new(20.0, -20.0),
        );
        app.update();
        touch_at(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(700.0, 120.0),
        );
        app.update();

        app.world_mut()
            .resource_mut::<AndroidShellState>()
            .lifecycle = AndroidLifecycle::Background;
        app.update();
        assert_eq!(app.world().resource::<JoystickState>().owner, None);
        assert_eq!(app.world().resource::<TouchPointer>().owner, None);
        assert!(!app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left));

        app.world_mut()
            .resource_mut::<AndroidShellState>()
            .lifecycle = AndroidLifecycle::Foreground;
        app.update();
        assert_eq!(app.world().resource::<JoystickState>().owner, None);
        assert_eq!(app.world().resource::<TouchPointer>().owner, None);
        assert!(app.world().resource::<TouchPointer>().wait_for_release);
    }

    #[test]
    fn ime_context_change_releases_the_pointer_until_the_touch_ends() {
        let (mut app, window) = joystick_app();
        touch_at(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(700.0, 120.0),
        );
        app.update();
        assert_eq!(app.world().resource::<TouchPointer>().owner, Some(2));

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .keyboard
            .open = true;
        app.update();

        let pointer = app.world().resource::<TouchPointer>();
        assert_eq!(pointer.owner, None);
        assert!(pointer.wait_for_release);
        assert!(!app
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
    fn npc_service_art_close_has_an_android_touch_target() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_systems(Startup, spawn);
        app.update();
        let world = app.world_mut();
        let targets: Vec<_> = world
            .query::<(&Name, &Node, &BackgroundColor, &Button)>()
            .iter(world)
            .filter(|(name, _, _, _)| name.as_str() == "AndroidNpcServiceCloseTarget")
            .collect();
        assert_eq!(
            targets.len(),
            1,
            "The red X baked into NPCDrop art is not a Button"
        );
        let (_, node, background, _) = targets[0];
        assert_eq!(
            node.display,
            Display::None,
            "Hidden until the real shared service opens"
        );
        assert_eq!(background.0, Color::NONE, "Do not replace the shared art");
        assert_eq!((node.width, node.height), (px(48.0), px(48.0)));
    }

    #[test]
    fn npc_close_target_keeps_phone_size_without_covering_service_actions() {
        use mir2_client_bevy::shop::{NpcShopServiceMode as Mode, NpcShopServiceSignal, ShopModel};
        let shell = NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        };
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
        for mode in [Mode::Buy, Mode::Sell, Mode::Repair, Mode::SpecialRepair] {
            let mut shop = ShopModel::default();
            assert!(shop.apply_service_signal(NpcShopServiceSignal {
                mode,
                repair_rate: matches!(mode, Mode::Repair | Mode::SpecialRepair).then_some(1.0),
            }));
            for (scale, panel_scale) in [(0.4, 0.7), (0.886, 1.0), (1.0, 1.0), (1.5, 0.8)] {
                let transform = UiTransform::from_scale(Vec2::splat(panel_scale));
                let node =
                    npc_service_close_node(&shell, &player, &shop, scale, &transform).unwrap();
                let (Val::Px(width), Val::Px(height), Val::Px(top)) =
                    (node.width, node.height, node.top)
                else {
                    panic!("Use source-local pixel geometry");
                };
                assert!((width * scale * panel_scale - 48.0).abs() < 0.0001);
                assert!((height * scale * panel_scale - 48.0).abs() < 0.0001);
                assert!(top + height <= if mode == Mode::Buy { 33.001 } else { 34.001 });
                let center_y = if mode == Mode::Buy { 13.5 } else { 12.0 };
                assert!(
                    top <= center_y && top + height >= center_y,
                    "Include the visible X"
                );
            }
        }
    }

    #[test]
    fn npc_close_target_reappears_after_shared_children_are_rebuilt() {
        use mir2_client_bevy::crystal_ui::overlays::OverlayShop;
        use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal, ShopModel};
        let mut app = App::new();
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
        let mut shop = ShopModel::default();
        shop.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        });
        app.add_plugins(MinimalPlugins)
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .insert_resource(player)
            .insert_resource(shop)
            .insert_resource(UiScale(0.886))
            .add_systems(Startup, spawn)
            .add_systems(Update, sync_npc_service_close_target);
        let root = app
            .world_mut()
            .spawn((OverlayShop, Node::default(), UiTransform::default()))
            .id();
        app.update();
        let old = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<(Entity, &ChildOf, &Node), With<NpcServiceCloseTarget>>();
            let (entity, parent, node) = query.single(world).unwrap();
            assert_eq!(parent.parent(), root);
            assert_eq!(node.display, Display::Flex);
            entity
        };
        app.world_mut()
            .entity_mut(root)
            .despawn_related::<Children>();
        app.update();
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(Entity, &ChildOf, &Node), With<NpcServiceCloseTarget>>();
        let (new, parent, node) = query.single(world).unwrap();
        assert_ne!(new, old);
        assert_eq!(parent.parent(), root);
        assert_eq!(node.display, Display::Flex);
        world
            .resource_mut::<NativePlayerUiState>()
            .close_all_windows();
        app.update();
        assert_eq!(app.world().get::<Node>(new).unwrap().display, Display::None);
    }

    #[test]
    fn npc_close_touch_uses_shared_cancellation_without_transaction_mutation() {
        use mir2_client_bevy::inventory::InventoryModel;
        let mut app = App::new();
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
        player.begin_npc_service_request();
        let inventory = InventoryModel {
            gold: 12345,
            ..default()
        };
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(player)
        .insert_resource(inventory.clone())
        .add_systems(Update, npc_service_close_buttons);
        let root = app
            .world_mut()
            .spawn((
                mir2_client_bevy::crystal_ui::overlays::OverlayShop,
                Node::default(),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.world_mut().spawn((
            NpcServiceCloseTarget,
            Interaction::Pressed,
            Node::default(),
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ));
        app.update();
        let player = app.world().resource::<NativePlayerUiState>();
        assert!(!player.npc_shop_open());
        assert!(!player.accepts_npc_service_reply());
        assert_eq!(
            serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap(),
            serde_json::to_value(&inventory).unwrap(),
        );
    }

    #[test]
    fn npc_close_cannot_consume_a_hidden_detached_or_old_parent_press() {
        use mir2_client_bevy::crystal_ui::overlays::OverlayShop;
        for context in 0..6 {
            let mut app = App::new();
            let mut player = NativePlayerUiState::default();
            player.core.screen = mir2_ui_core::state::UiScreen::InGame;
            player.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
            player.begin_npc_service_request();
            app.insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .insert_resource(player)
            .add_systems(Update, npc_service_close_buttons);
            let root = app
                .world_mut()
                .spawn((
                    OverlayShop,
                    Node {
                        display: if context == 3 {
                            Display::None
                        } else {
                            Display::Flex
                        },
                        ..default()
                    },
                    if context == 5 {
                        InheritedVisibility::HIDDEN
                    } else {
                        InheritedVisibility::VISIBLE
                    },
                ))
                .id();
            let target = app
                .world_mut()
                .spawn((
                    NpcServiceCloseTarget,
                    Interaction::Pressed,
                    Node {
                        display: if context == 0 {
                            Display::None
                        } else {
                            Display::Flex
                        },
                        ..default()
                    },
                    if context == 4 {
                        InheritedVisibility::HIDDEN
                    } else {
                        InheritedVisibility::VISIBLE
                    },
                ))
                .id();
            match context {
                1 => {} // detached target
                2 => {
                    let old = app
                        .world_mut()
                        .spawn((Node::default(), InheritedVisibility::VISIBLE))
                        .id();
                    app.world_mut().entity_mut(target).insert(ChildOf(old));
                }
                _ => {
                    app.world_mut().entity_mut(target).insert(ChildOf(root));
                }
            }
            app.update();
            let player = app.world().resource::<NativePlayerUiState>();
            assert!(
                player.npc_shop_open(),
                "context {context} cannot close the current shared service"
            );
            assert!(
                player.accepts_npc_service_reply(),
                "context {context} cannot cancel the new request"
            );
        }
    }

    #[test]
    fn npc_close_target_rejects_hidden_closed_and_modal_contexts() {
        use mir2_client_bevy::inventory::{InventoryModel, ItemModel};
        use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal, ShopModel};
        let mut shop = ShopModel::default();
        shop.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        });
        for context in 0..5 {
            let mut shell = NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            };
            let mut player = NativePlayerUiState::default();
            player.core.screen = mir2_ui_core::state::UiScreen::InGame;
            player.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
            match context {
                0 => shell.screen = NativeShellScreen::Login,
                1 => player.close_all_windows(),
                2 => player.keyboard.open = true,
                3 => player.trade_dialog.open = true,
                _ => {
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
                    assert!(player.open_inventory_delete_for_slot(&inventory, 0));
                }
            }
            assert!(
                npc_service_close_node(&shell, &player, &shop, 1.0, &UiTransform::default())
                    .is_none()
            );
            let before_panel = player.core.panel;
            let before_pending = player.accepts_npc_service_reply();
            let mut app = App::new();
            app.insert_resource(shell)
                .insert_resource(player)
                .add_systems(Update, npc_service_close_buttons);
            let root = app
                .world_mut()
                .spawn((
                    mir2_client_bevy::crystal_ui::overlays::OverlayShop,
                    Node::default(),
                    InheritedVisibility::VISIBLE,
                ))
                .id();
            app.world_mut().spawn((
                NpcServiceCloseTarget,
                Interaction::Pressed,
                Node::default(),
                InheritedVisibility::VISIBLE,
                ChildOf(root),
            ));
            app.update();
            let after = app.world().resource::<NativePlayerUiState>();
            assert_eq!(after.core.panel, before_panel);
            assert_eq!(after.accepts_npc_service_reply(), before_pending);
        }
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
        {
            let world = app.world_mut();
            let mut rail = world.query_filtered::<&Node, With<TouchRail>>();
            assert_eq!(rail.single(world).unwrap().display, Display::None);
        }
        {
            let world = app.world_mut();
            let mut pad = world.query_filtered::<&Node, With<ActionPad>>();
            assert_eq!(pad.single(world).unwrap().display, Display::Flex);
        }
        {
            let world = app.world_mut();
            let mut joystick = world.query_filtered::<&Node, With<JoystickRoot>>();
            assert_eq!(joystick.single(world).unwrap().display, Display::Flex);
        }
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
        app.world_mut().resource_mut::<RailState>().expanded = true;
        app.update();
        {
            let world = app.world_mut();
            let mut rail = world.query_filtered::<&Node, With<TouchRail>>();
            assert_eq!(rail.single(world).unwrap().display, Display::Flex);
            let mut pad = world.query_filtered::<&Node, With<ActionPad>>();
            assert_eq!(pad.single(world).unwrap().display, Display::None);
            let mut joystick = world.query_filtered::<&Node, With<JoystickRoot>>();
            assert_eq!(joystick.single(world).unwrap().display, Display::None);
        }
        app.world_mut().resource_mut::<RailState>().expanded = false;
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Inventory;
        app.update();
        {
            let world = app.world_mut();
            let mut pad = world.query_filtered::<&Node, With<ActionPad>>();
            assert_eq!(pad.single(world).unwrap().display, Display::Flex);
        }
        {
            let world = app.world_mut();
            let mut joystick = world.query_filtered::<&Node, With<JoystickRoot>>();
            assert_eq!(joystick.single(world).unwrap().display, Display::Flex);
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::None;
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

    #[test]
    fn gameplay_controls_follow_short_edge_and_keep_button_height() {
        let normal_phone = gameplay_control_metrics(393.0);
        assert!(((normal_phone.joystick_scale * JOYSTICK_DIAMETER) - 62.88).abs() < 0.001);
        assert_eq!(normal_phone.button_height, CONTROL_MIN_BUTTON_HEIGHT);
        assert!(normal_phone.action_pad_width < 120.0);

        let compact_phone = gameplay_control_metrics(262.0);
        assert_eq!(
            compact_phone.joystick_scale * JOYSTICK_DIAMETER,
            JOYSTICK_MIN_DIAMETER
        );
        assert_eq!(compact_phone.action_pad_width, 210.0);
        assert_eq!(compact_phone.button_width, 48.0);
        assert_eq!(compact_phone.button_height, CONTROL_MIN_BUTTON_HEIGHT);
        assert_eq!(compact_phone.rail_width, 204.0);
        assert_eq!(compact_phone.rail_button_width, 48.0);

        let tablet = gameplay_control_metrics(1_080.0);
        assert_eq!(tablet.joystick_scale, 1.0);
        assert_eq!(tablet.action_pad_width, 144.0);
    }
}
