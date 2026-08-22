//! Android gesture and lifecycle routing.
//!
//! A renderer/Activity performs hit-testing and sends semantic targets here.
//! An unclassified touch is ignored; it never becomes `OpenInventory`.

use bevy::prelude::*;
use mir2_ui_core::{
    action::UiAction,
    state::{UiPanel, UiState},
};
use serde::{Deserialize, Serialize};

use crate::gateway_bridge::AndroidGatewayOutboundQueue;

const JOYSTICK_DEAD_ZONE_SQUARED: f32 = 0.0225;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct AndroidInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl AndroidInsets {
    fn sanitized(self) -> Self {
        Self {
            left: self.left.max(0.0),
            top: self.top.max(0.0),
            right: self.right.max(0.0),
            bottom: self.bottom.max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AndroidOrientation {
    #[default]
    Unknown,
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AndroidLifecycle {
    #[default]
    Created,
    Foreground,
    Background,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AndroidNetwork {
    #[default]
    Unknown,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Resource)]
pub struct AndroidShellState {
    pub lifecycle: AndroidLifecycle,
    pub network: AndroidNetwork,
    pub orientation: AndroidOrientation,
    pub width: f32,
    pub height: f32,
    pub safe_area: AndroidInsets,
    pub retry_when_online: bool,
}

impl Default for AndroidShellState {
    fn default() -> Self {
        Self {
            lifecycle: AndroidLifecycle::Created,
            network: AndroidNetwork::Unknown,
            orientation: AndroidOrientation::Unknown,
            width: 0.0,
            height: 0.0,
            safe_area: AndroidInsets::default(),
            retry_when_online: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AndroidLifecycleEvent {
    Resume,
    Pause,
    Destroy,
    NetworkAvailable,
    NetworkUnavailable,
    WindowMetrics { width: f32, height: f32 },
    SafeAreaChanged(AndroidInsets),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AndroidLifecycleEffect {
    ResumeRendering,
    SuspendRendering,
    PauseNetwork,
    ResumeNetwork,
    SaveTransientState,
    RetryConnection,
    ViewportChanged,
}

impl AndroidShellState {
    /// Pure lifecycle reducer; the eventual gateway/Activity host owns effects.
    pub fn apply_lifecycle(&mut self, event: AndroidLifecycleEvent) -> Vec<AndroidLifecycleEffect> {
        match event {
            AndroidLifecycleEvent::Resume => {
                if self.lifecycle == AndroidLifecycle::Destroyed {
                    return Vec::new();
                }
                self.lifecycle = AndroidLifecycle::Foreground;
                let mut effects = vec![AndroidLifecycleEffect::ResumeRendering];
                if self.network == AndroidNetwork::Available {
                    effects.push(AndroidLifecycleEffect::ResumeNetwork);
                    if self.retry_when_online {
                        self.retry_when_online = false;
                        effects.push(AndroidLifecycleEffect::RetryConnection);
                    }
                }
                effects
            }
            AndroidLifecycleEvent::Pause => {
                if self.lifecycle != AndroidLifecycle::Foreground {
                    return Vec::new();
                }
                self.lifecycle = AndroidLifecycle::Background;
                vec![
                    AndroidLifecycleEffect::SaveTransientState,
                    AndroidLifecycleEffect::SuspendRendering,
                    AndroidLifecycleEffect::PauseNetwork,
                ]
            }
            AndroidLifecycleEvent::Destroy => {
                if self.lifecycle == AndroidLifecycle::Destroyed {
                    return Vec::new();
                }
                self.lifecycle = AndroidLifecycle::Destroyed;
                vec![
                    AndroidLifecycleEffect::SaveTransientState,
                    AndroidLifecycleEffect::SuspendRendering,
                    AndroidLifecycleEffect::PauseNetwork,
                ]
            }
            AndroidLifecycleEvent::NetworkAvailable => {
                self.network = AndroidNetwork::Available;
                if self.lifecycle != AndroidLifecycle::Foreground {
                    return Vec::new();
                }
                let mut effects = vec![AndroidLifecycleEffect::ResumeNetwork];
                if self.retry_when_online {
                    self.retry_when_online = false;
                    effects.push(AndroidLifecycleEffect::RetryConnection);
                }
                effects
            }
            AndroidLifecycleEvent::NetworkUnavailable => {
                self.network = AndroidNetwork::Unavailable;
                self.retry_when_online = true;
                if self.lifecycle == AndroidLifecycle::Foreground {
                    vec![AndroidLifecycleEffect::PauseNetwork]
                } else {
                    Vec::new()
                }
            }
            AndroidLifecycleEvent::WindowMetrics { width, height } => {
                self.width = width.max(0.0);
                self.height = height.max(0.0);
                self.orientation = if self.width == 0.0 || self.height == 0.0 {
                    AndroidOrientation::Unknown
                } else if self.width >= self.height {
                    AndroidOrientation::Landscape
                } else {
                    AndroidOrientation::Portrait
                };
                vec![AndroidLifecycleEffect::ViewportChanged]
            }
            AndroidLifecycleEvent::SafeAreaChanged(insets) => {
                self.safe_area = insets.sanitized();
                vec![AndroidLifecycleEffect::ViewportChanged]
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidUiTarget {
    None,
    Inventory,
    Character,
    Skill,
    QuestLog,
    Options,
    ChatSettings,
    Menu,
    Mail,
    BigMap,
    Storage,
    Minimap,
    Chat,
    InventoryItem { unique_id: u64 },
    Npc { object_id: u32 },
    AttackTarget { object_id: u32 },
    PickUp { object_id: u32 },
    TownRevive,
}

impl AndroidUiTarget {
    fn action(&self) -> Option<UiAction> {
        Some(match self {
            Self::None => return None,
            Self::Inventory => UiAction::OpenInventory,
            Self::Character => UiAction::OpenCharacter,
            Self::Skill => UiAction::OpenSkill,
            Self::QuestLog => UiAction::OpenQuestLog,
            Self::Options => UiAction::OpenOptions,
            Self::ChatSettings => UiAction::OpenChatSettings,
            Self::Menu => UiAction::OpenMenu,
            Self::Mail => UiAction::OpenMail,
            Self::BigMap => UiAction::OpenBigMap,
            Self::Storage => UiAction::OpenStorage,
            Self::Minimap => UiAction::ToggleMinimap,
            Self::Chat => UiAction::FocusChat,
            Self::InventoryItem { unique_id } => UiAction::UseItem {
                unique_id: *unique_id,
            },
            Self::Npc { object_id } => UiAction::InteractNpc {
                object_id: *object_id,
            },
            Self::AttackTarget { object_id } => UiAction::AttackTarget {
                object_id: *object_id,
            },
            Self::PickUp { object_id } => UiAction::PickUp {
                object_id: *object_id,
            },
            Self::TownRevive => UiAction::TownRevive,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidSoftKeyboardEvent {
    FocusChat,
    Dismiss,
    Submit { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidDirection {
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidMoveMode {
    Walk,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidMotionIntent {
    pub direction: AndroidDirection,
    pub mode: AndroidMoveMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AndroidInputEvent {
    Tap {
        target: AndroidUiTarget,
    },
    /// A renderer/Activity may already have performed hit-testing and emit
    /// the platform-agnostic action directly. This keeps Android on the same
    /// reducer/effect vector as Windows without duplicating UI state logic.
    Semantic(UiAction),
    LongPress {
        target: AndroidUiTarget,
    },
    Drag {
        target: AndroidUiTarget,
        delta_y: f32,
    },
    Back,
    SoftKeyboard(AndroidSoftKeyboardEvent),
    /// UiAction currently has no Walk/Run/Turn variant. Keep this explicit so
    /// joystick input cannot be faked as Inventory or any other UI action.
    VirtualJoystick {
        x: f32,
        y: f32,
        run: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AndroidInputRoute {
    UiAction(UiAction),
    Motion(AndroidMotionIntent),
}

pub fn route_input(event: AndroidInputEvent, state: &UiState) -> Vec<AndroidInputRoute> {
    let ui = |action: Option<UiAction>| -> Vec<AndroidInputRoute> {
        action
            .map(AndroidInputRoute::UiAction)
            .into_iter()
            .collect()
    };
    match event {
        AndroidInputEvent::Semantic(action) => {
            vec![AndroidInputRoute::UiAction(action)]
        }
        AndroidInputEvent::Tap { target } | AndroidInputEvent::LongPress { target } => {
            ui(target.action())
        }
        AndroidInputEvent::Drag {
            target: AndroidUiTarget::Chat,
            delta_y,
        } if delta_y < 0.0 => {
            vec![AndroidInputRoute::UiAction(UiAction::ScrollChatUp)]
        }
        AndroidInputEvent::Drag {
            target: AndroidUiTarget::Chat,
            ..
        } => {
            vec![AndroidInputRoute::UiAction(UiAction::ScrollChatDown)]
        }
        AndroidInputEvent::Drag { .. } => Vec::new(),
        AndroidInputEvent::Back => {
            let action = if state.chat_focused {
                UiAction::BlurChat
            } else if state.panel != UiPanel::None {
                UiAction::ClosePanel
            } else if state.minimap_visible {
                UiAction::ToggleMinimap
            } else {
                UiAction::ExitApplication
            };
            vec![AndroidInputRoute::UiAction(action)]
        }
        AndroidInputEvent::SoftKeyboard(AndroidSoftKeyboardEvent::FocusChat) => {
            vec![AndroidInputRoute::UiAction(UiAction::FocusChat)]
        }
        AndroidInputEvent::SoftKeyboard(AndroidSoftKeyboardEvent::Dismiss) => {
            vec![AndroidInputRoute::UiAction(UiAction::BlurChat)]
        }
        AndroidInputEvent::SoftKeyboard(AndroidSoftKeyboardEvent::Submit { message }) => {
            let message = message.trim();
            if message.is_empty() {
                Vec::new()
            } else {
                vec![
                    AndroidInputRoute::UiAction(UiAction::SendChat {
                        message: message.to_owned(),
                    }),
                    AndroidInputRoute::UiAction(UiAction::BlurChat),
                ]
            }
        }
        AndroidInputEvent::VirtualJoystick { x, y, run } => {
            if x * x + y * y < JOYSTICK_DEAD_ZONE_SQUARED {
                return Vec::new();
            }
            let h = if x > 0.15 {
                1
            } else if x < -0.15 {
                -1
            } else {
                0
            };
            let v = if y > 0.15 {
                1
            } else if y < -0.15 {
                -1
            } else {
                0
            };
            let direction = match (h, v) {
                (0, -1) => AndroidDirection::Up,
                (1, -1) => AndroidDirection::UpRight,
                (1, 0) => AndroidDirection::Right,
                (1, 1) => AndroidDirection::DownRight,
                (0, 1) => AndroidDirection::Down,
                (-1, 1) => AndroidDirection::DownLeft,
                (-1, 0) => AndroidDirection::Left,
                (-1, -1) => AndroidDirection::UpLeft,
                _ => return Vec::new(),
            };
            vec![AndroidInputRoute::Motion(AndroidMotionIntent {
                direction,
                mode: if run {
                    AndroidMoveMode::Run
                } else {
                    AndroidMoveMode::Walk
                },
            })]
        }
    }
}

#[derive(Debug, Clone, Message)]
pub struct AndroidInputMessage(pub AndroidInputEvent);
#[derive(Debug, Clone, Message)]
pub struct AndroidLifecycleMessage(pub AndroidLifecycleEvent);
#[derive(Debug, Default, Resource)]
pub struct AndroidUiActionQueue(pub Vec<UiAction>);
#[derive(Debug, Default, Resource)]
pub struct AndroidMotionQueue(pub Vec<AndroidMotionIntent>);
#[derive(Debug, Default, Resource)]
pub struct AndroidLifecycleEffects(pub Vec<AndroidLifecycleEffect>);

pub fn collect_android_back_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<AndroidInputMessage>,
) {
    if keys.just_pressed(KeyCode::BrowserBack) || keys.just_pressed(KeyCode::Escape) {
        writer.write(AndroidInputMessage(AndroidInputEvent::Back));
    }
}

pub fn route_android_input_messages(
    mut reader: MessageReader<AndroidInputMessage>,
    state: Res<UiState>,
    mut actions: ResMut<AndroidUiActionQueue>,
    mut motions: ResMut<AndroidMotionQueue>,
) {
    for message in reader.read() {
        for route in route_input(message.0.clone(), &state) {
            match route {
                AndroidInputRoute::UiAction(action) => actions.0.push(action),
                AndroidInputRoute::Motion(intent) => motions.0.push(intent),
            }
        }
    }
}

pub fn apply_android_lifecycle_messages(
    mut reader: MessageReader<AndroidLifecycleMessage>,
    mut shell: ResMut<AndroidShellState>,
    mut effects: ResMut<AndroidLifecycleEffects>,
    mut actions: ResMut<AndroidUiActionQueue>,
    mut gateway: ResMut<AndroidGatewayOutboundQueue>,
    mut ui_state: ResMut<UiState>,
) {
    for message in reader.read() {
        let terminal = matches!(message.0, AndroidLifecycleEvent::Destroy);
        let next = shell.apply_lifecycle(message.0.clone());
        if terminal {
            gateway.mark_terminal_reset();
            ui_state.mark_game_shop_unknown();
        }
        if next
            .iter()
            .any(|effect| matches!(effect, AndroidLifecycleEffect::RetryConnection))
        {
            actions.0.push(UiAction::RetryConnection);
        }
        effects.0.extend(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_ui_core::state::UiScreen;

    fn game_state() -> UiState {
        UiState {
            screen: UiScreen::InGame,
            ..Default::default()
        }
    }

    #[test]
    fn semantic_taps_do_not_all_open_inventory() {
        let state = game_state();
        assert_eq!(
            route_input(
                AndroidInputEvent::Tap {
                    target: AndroidUiTarget::Mail
                },
                &state
            ),
            vec![AndroidInputRoute::UiAction(UiAction::OpenMail)]
        );
        assert!(
            route_input(
                AndroidInputEvent::Tap {
                    target: AndroidUiTarget::None
                },
                &state
            )
            .is_empty()
        );
    }

    #[test]
    fn long_press_drag_and_keyboard_use_shared_actions() {
        let state = game_state();
        assert_eq!(
            route_input(
                AndroidInputEvent::LongPress {
                    target: AndroidUiTarget::InventoryItem { unique_id: 9 }
                },
                &state
            ),
            vec![AndroidInputRoute::UiAction(UiAction::UseItem {
                unique_id: 9
            })]
        );
        assert_eq!(
            route_input(
                AndroidInputEvent::Drag {
                    target: AndroidUiTarget::Chat,
                    delta_y: -3.0
                },
                &state
            ),
            vec![AndroidInputRoute::UiAction(UiAction::ScrollChatUp)]
        );
        assert_eq!(
            route_input(
                AndroidInputEvent::SoftKeyboard(AndroidSoftKeyboardEvent::Submit {
                    message: "  hi  ".into()
                }),
                &state
            ),
            vec![
                AndroidInputRoute::UiAction(UiAction::SendChat {
                    message: "hi".into()
                }),
                AndroidInputRoute::UiAction(UiAction::BlurChat)
            ]
        );
    }

    #[test]
    fn back_closes_keyboard_then_panel_then_requests_exit() {
        let mut state = game_state();
        state.chat_focused = true;
        assert_eq!(
            route_input(AndroidInputEvent::Back, &state),
            vec![AndroidInputRoute::UiAction(UiAction::BlurChat)]
        );
        state.chat_focused = false;
        state.panel = UiPanel::Mail;
        assert_eq!(
            route_input(AndroidInputEvent::Back, &state),
            vec![AndroidInputRoute::UiAction(UiAction::ClosePanel)]
        );
        state.panel = UiPanel::None;
        state.minimap_visible = false;
        assert_eq!(
            route_input(AndroidInputEvent::Back, &state),
            vec![AndroidInputRoute::UiAction(UiAction::ExitApplication)]
        );
    }

    #[test]
    fn joystick_is_motion_not_a_fake_inventory_action() {
        let state = game_state();
        assert_eq!(
            route_input(
                AndroidInputEvent::VirtualJoystick {
                    x: 0.8,
                    y: -0.8,
                    run: true
                },
                &state
            ),
            vec![AndroidInputRoute::Motion(AndroidMotionIntent {
                direction: AndroidDirection::UpRight,
                mode: AndroidMoveMode::Run
            })]
        );
    }

    #[test]
    fn lifecycle_retries_only_after_network_recovers_in_foreground() {
        let mut shell = AndroidShellState::default();
        assert_eq!(
            shell.apply_lifecycle(AndroidLifecycleEvent::Resume),
            vec![AndroidLifecycleEffect::ResumeRendering]
        );
        assert_eq!(
            shell.apply_lifecycle(AndroidLifecycleEvent::NetworkUnavailable),
            vec![AndroidLifecycleEffect::PauseNetwork]
        );
        assert_eq!(
            shell.apply_lifecycle(AndroidLifecycleEvent::NetworkAvailable),
            vec![
                AndroidLifecycleEffect::ResumeNetwork,
                AndroidLifecycleEffect::RetryConnection
            ]
        );
    }

    #[test]
    fn safe_area_and_landscape_metrics_are_retained() {
        let mut shell = AndroidShellState::default();
        shell.apply_lifecycle(AndroidLifecycleEvent::WindowMetrics {
            width: 2400.0,
            height: 1080.0,
        });
        shell.apply_lifecycle(AndroidLifecycleEvent::SafeAreaChanged(AndroidInsets {
            left: -1.0,
            top: 40.0,
            right: 50.0,
            bottom: -2.0,
        }));
        assert_eq!(shell.orientation, AndroidOrientation::Landscape);
        assert_eq!(
            shell.safe_area,
            AndroidInsets {
                left: 0.0,
                top: 40.0,
                right: 50.0,
                bottom: 0.0
            }
        );
    }
}
