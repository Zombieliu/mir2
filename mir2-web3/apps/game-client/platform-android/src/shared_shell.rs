//! Android adapter for the SAME Crystal shell used by the Windows host.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::{login::CrystalLoginAction, CrystalStageTransform},
    native_shell::{
        CharacterSummary, NativeGatewayEvent as Event, NativeShellModel,
        NativeShellScreen as Screen, NativeUiIntent as Intent, NativeUiIntentQueue, ShellNotice,
    },
    native_shell_ui::{Mir2NativeShellUiPlugin, NativeShellRoot},
};
use serde_json::{json, Value};
use std::{collections::VecDeque, sync::Mutex};

static INBOX: Mutex<VecDeque<Value>> = Mutex::new(VecDeque::new());
static OUTBOX: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mir2_web3_MainActivity_nativeEvent<'a>(
    mut env: jni::EnvUnowned<'a>,
    _: jni::objects::JClass<'a>,
    text: jni::objects::JString<'a>,
) {
    env.with_env(|_| -> Result<(), jni::errors::Error> {
        let text = text.to_string();
        if text.len() <= 65536 {
            if let Ok(value) = serde_json::from_str(&text) {
                let mut queue = INBOX.lock().unwrap_or_else(|e| e.into_inner());
                if queue.len() >= 32 {
                    queue.clear();
                    queue.push_back(
                        json!({"phase":"DISCONNECTED","message":"Host event overflow; reconnect"}),
                    );
                } else {
                    queue.push_back(value);
                }
            }
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mir2_web3_MainActivity_nativePoll<'a>(
    mut env: jni::EnvUnowned<'a>,
    _: jni::objects::JClass<'a>,
) -> jni::sys::jstring {
    env.with_env(|env| -> Result<_, jni::errors::Error> {
        let command = OUTBOX
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
            .unwrap_or_default();
        Ok(env.new_string(command)?.into_raw())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn send(value: Value) {
    let mut queue = OUTBOX.lock().unwrap_or_else(|e| e.into_inner());
    if queue.len() < 16 {
        queue.push_back(value.to_string());
    }
}

#[derive(Resource, Default)]
pub(crate) struct HostState {
    phase: String,
    world: Option<HostWorldPosition>,
    ime_bottom: f32,
    pub(crate) safe_right: f32,
    pub(crate) safe_top: f32,
    safe_left: f32,
    safe_bottom: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostWorldPosition {
    player_name: String,
    map_file_name: String,
    x: u32,
    y: u32,
}

fn host_world_position(value: &Value, screen: Screen) -> Option<HostWorldPosition> {
    if value["phase"] != "IN_GAME" || !matches!(screen, Screen::StartingGame | Screen::InGame) {
        return None;
    }
    let world: HostWorldPosition = serde_json::from_value(value["world"].clone()).ok()?;
    if [&world.player_name, &world.map_file_name]
        .iter()
        .any(|s| s.trim().is_empty() || s.chars().count() > 128)
        || world.x > i32::MAX as u32
        || world.y > i32::MAX as u32
    {
        return None;
    }
    Some(world)
}

#[derive(Resource, Default)]
struct EditorTouch(bool);

fn remember_editor_touch(
    mut touch: ResMut<EditorTouch>,
    mut held: Local<bool>,
    targets: Query<
        &Interaction,
        With<mir2_client_bevy::crystal_ui::overlays::NativeTextInputTarget>,
    >,
) {
    // Capture before Update rebuilds overlay children. Querying those children
    // in PostUpdate loses the press when the focused field did not change.
    let pressed = targets
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    touch.0 = pressed && !*held;
    *held = pressed;
}

pub struct AndroidSharedShellPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AndroidStageFit;
impl Plugin for AndroidSharedShellPlugin {
    fn build(&self, app: &mut App) {
        let mut model = NativeShellModel::default();
        model.screen = Screen::Login;
        model.notice = Some(ShellNotice::info(
            "Configure an approved test Gateway before connecting.",
        ));
        app.insert_resource(model)
            .init_resource::<HostState>()
            .init_resource::<EditorTouch>()
            .add_plugins(Mir2NativeShellUiPlugin)
            .add_plugins((
                mir2_client_bevy::crystal_ui::minimap::Mir2CrystalMiniMapPlugin,
                mir2_client_bevy::crystal_ui::hud::Mir2CrystalHudPlugin,
                mir2_client_bevy::crystal_ui::chat::Mir2CrystalChatPlugin,
                mir2_client_bevy::crystal_ui::notice::Mir2CrystalNoticePlugin,
                mir2_client_bevy::quest_ui::Mir2QuestUiPlugin,
            ))
            .add_systems(
                PreUpdate,
                (receive, discard_inactive_player_commands)
                    .chain()
                    .before(bevy::input::InputSystems),
            )
            .add_systems(
                PreUpdate,
                remember_editor_touch.after(bevy::ui::UiSystems::Focus),
            )
            .add_systems(
                PostUpdate,
                (
                    fit_stage.in_set(AndroidStageFit),
                    fit_mail_composer,
                    forward_intents,
                    discard_inactive_player_commands,
                    keyboard,
                )
                    .chain()
                    .before(bevy::ui::UiSystems::Layout),
            );
        #[cfg(feature = "ui-preview")]
        crate::ui_preview::install(app);
        crate::mobile_ui::install(app);
    }
}

fn fit_stage(
    windows: Query<&Window>,
    host: Res<HostState>,
    model: Res<NativeShellModel>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    forms: crate::form_input::FormInput,
    mut scale: ResMut<UiScale>,
    belt: Res<mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation>,
    mut roots: Query<
        (
            &mut Node,
            Has<mir2_client_bevy::crystal_ui::minimap::CrystalMiniMapRoot>,
        ),
        (
            Or<(
                With<NativeShellRoot>,
                With<mir2_client_bevy::crystal_ui::hud::CrystalHudRoot>,
                With<mir2_client_bevy::crystal_ui::chat::CrystalChatRoot>,
                With<mir2_client_bevy::crystal_ui::notice::CrystalNoticeRoot>,
                With<mir2_client_bevy::crystal_ui::minimap::CrystalMiniMapRoot>,
                With<mir2_client_bevy::crystal_ui::overlays::OverlayRoot>,
                With<mir2_client_bevy::quest_ui::QuestUiRoot>,
            )>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudMiniMapLayer>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>,
        ),
    >,
    mut minimap_layers: Query<
        &mut Node,
        (
            With<mir2_client_bevy::crystal_ui::hud::CrystalHudMiniMapLayer>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>,
        ),
    >,
    mut belt_layers: Query<&mut Node, With<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let fit = CrystalStageTransform::fit(window.width(), window.height());
    // Keep field/button size stable while typing. Pan the shared stage to keep
    // its login panel above the keyboard instead of shrinking it to a thumbnail.
    let available = (window.height() - host.ime_bottom / window.scale_factor()).max(1.0);
    let panel = mir2_client_bevy::crystal_ui::spec::login::PANEL.rect;
    let top = if host.ime_bottom > 0.0 && model.screen == Screen::InGame {
        let field = forms
            .field(&player)
            .or_else(|| crate::text_input::player_field(&player));
        let editor_bottom = player_editor_bottom(field.map(|field| field.0), fit.scale);
        (available / fit.scale - editor_bottom).min(fit.offset_y / fit.scale)
    } else if host.ime_bottom > 0.0 {
        (available / (2.0 * fit.scale) - panel.top - panel.height * 0.5)
            .min(fit.offset_y / fit.scale)
    } else {
        fit.offset_y / fit.scale
    };
    scale.0 = fit.scale;
    let map_origin = minimap_edge_origin(
        window.width(),
        window.scale_factor(),
        fit.scale,
        host.safe_right,
        host.safe_top,
    );
    for (mut root, is_map_image) in &mut roots {
        root.left = px(if is_map_image {
            map_origin.x
        } else {
            fit.offset_x / fit.scale
        });
        root.top = px(if is_map_image { map_origin.y } else { top });
    }
    // The frame/actions are children of the centered HUD; the map image is
    // a separate root. Compensate the parent transform so they stay aligned,
    // even while the dialog stage pans for IME. Bevy hits the moved nodes.
    for mut layer in &mut minimap_layers {
        layer.left = px(map_origin.x - fit.offset_x / fit.scale);
        layer.top = px(map_origin.y - top);
    }
    let belt_origin = belt_edge_origin(
        fit,
        window.height(),
        host.safe_left / window.scale_factor(),
        host.safe_bottom / window.scale_factor(),
        belt.vertical,
    );
    for mut layer in &mut belt_layers {
        let offset = belt_origin
            .map(|origin| origin - Vec2::new(fit.offset_x / fit.scale, top))
            .unwrap_or(Vec2::ZERO);
        layer.left = px(offset.x);
        layer.top = px(offset.y);
        // Editing must not leave relocated item buttons active over the IME.
        // The user's shared visible/orientation preference is left unchanged.
        layer.display = if host.ime_bottom > 0.0 {
            Display::None
        } else {
            Display::Flex
        };
    }
}

// Keep the two text fields visible while IME is open. Temporarily collapse
// attachment/gold controls and move the SAME shared footer under the fields;
// do not pan the entire 444px panel and push the recipient off screen.
fn fit_mail_composer(
    host: Res<HostState>,
    scale: Res<UiScale>,
    mut details: Query<
        &mut Node,
        (
            With<mir2_client_bevy::crystal_ui::overlays::MailComposeDetails>,
            Without<mir2_client_bevy::crystal_ui::overlays::MailComposeFooter>,
            Without<Button>,
        ),
    >,
    mut footers: Query<
        (&mut Node, &Children),
        (
            With<mir2_client_bevy::crystal_ui::overlays::MailComposeFooter>,
            Without<Button>,
        ),
    >,
    mut buttons: Query<
        &mut Node,
        (
            Without<mir2_client_bevy::crystal_ui::overlays::MailComposeDetails>,
            Without<mir2_client_bevy::crystal_ui::overlays::MailComposeFooter>,
        ),
    >,
) {
    let editing = host.ime_bottom > 0.0;
    for mut node in &mut details {
        node.display = if editing {
            Display::None
        } else {
            Display::Flex
        };
    }
    for (mut node, children) in &mut footers {
        node.top = px(if editing { 110.0 - 408.0 } else { 0.0 });
        for child in children.iter() {
            if let Ok(mut button) = buttons.get_mut(child) {
                button.height = px(if editing {
                    (44.0 / scale.0.max(0.01)).max(28.0)
                } else {
                    28.0
                });
            }
        }
    }
}

fn belt_edge_origin(
    fit: CrystalStageTransform,
    height: f32,
    safe_left: f32,
    safe_bottom: f32,
    vertical: bool,
) -> Option<Vec2> {
    let frame = mir2_client_bevy::crystal_ui::hud::belt_frame_rect(vertical);
    let left = safe_left + 8.0;
    if left + frame.width * fit.scale + 8.0 > fit.offset_x {
        return None; // Keep the source layout when the side gutter cannot fit it.
    }
    Some(Vec2::new(
        left / fit.scale - frame.left,
        (height - safe_bottom - 8.0) / fit.scale - frame.top - frame.height,
    ))
}

fn minimap_edge_origin(width: f32, dpi: f32, scale: f32, safe_right: f32, safe_top: f32) -> Vec2 {
    Vec2::new(
        (width - safe_right / dpi - 8.0) / scale - 1024.0,
        (safe_top / dpi + 8.0) / scale,
    )
}

fn player_editor_bottom(field: Option<&str>, scale: f32) -> f32 {
    match field {
        Some("guild-notice") => {
            // Shared notice text starts at panel +61, in 9px type. Reserve
            // 12px per permitted line plus a gutter, not the bottom HUD row.
            mir2_client_bevy::crystal_ui::overlays::CRYSTAL_GUILD_PANEL_RECT.top
                + 61.0
                + mir2_client_bevy::social::MAX_NOTICE_LINES as f32 * 12.0
                + 8.0
        }
        Some("inventory-amount" | "guild-amount" | "trade-amount") => {
            let rect = mir2_client_bevy::crystal_ui::overlays::CRYSTAL_DELETE_AMOUNT_RECT;
            rect.top + rect.height + 8.0
        }
        // Mail panel top + compact footer top + touch target + IME gutter.
        Some("mail-recipient" | "mail-message") => {
            5.0 + 110.0 + (44.0 / scale.max(0.01)).max(28.0) + 8.0 / scale.max(0.01)
        }
        _ => 750.0,
    }
}

fn receive(
    mut model: ResMut<NativeShellModel>,
    mut host: ResMut<HostState>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
    mut intents: ResMut<NativeUiIntentQueue>,
    mut player: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
    mut key_events: Option<ResMut<Messages<bevy::input::keyboard::KeyboardInput>>>,
    windows: Query<Entity, With<Window>>,
    mut forms: crate::form_input::FormInput,
    #[cfg(feature = "ui-preview")] mut preview: ResMut<crate::ui_preview::PreviewRequest>,
) {
    let values: Vec<_> = INBOX
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect();
    for value in values {
        if value["type"] == "submit" {
            let active = crate::text_input::shell_field(&model)
                .map(|v| v.0)
                .or_else(|| {
                    player
                        .as_deref()
                        .filter(|_| model.screen == Screen::InGame)
                        .and_then(|p| {
                            forms
                                .field(p)
                                .or_else(|| crate::text_input::player_field(p))
                        })
                        .map(|v| v.0)
                });
            if active.is_some() && active == value["field"].as_str() {
                if let (Some(events), Ok(window)) = (key_events.as_deref_mut(), windows.single()) {
                    use bevy::input::{
                        keyboard::{Key, KeyboardInput},
                        ButtonState,
                    };
                    for state in [ButtonState::Pressed, ButtonState::Released] {
                        events.write(KeyboardInput {
                            key_code: KeyCode::Enter,
                            logical_key: Key::Enter,
                            state,
                            text: None,
                            repeat: false,
                            window,
                        });
                    }
                }
            }
            continue;
        }
        if value["type"] == "back" {
            match model.screen {
                Screen::InGame => {
                    if let (Some(events), Ok(window)) =
                        (key_events.as_deref_mut(), windows.single())
                    {
                        shared_back_key(events, window);
                    }
                }
                Screen::CharacterCreate => {
                    model.apply_ui_intent(Intent::CancelCharacterCreate);
                }
                Screen::ChangePassword => {
                    model.apply_ui_intent(Intent::CancelChangePassword);
                }
                Screen::SafeKey => {
                    model.apply_ui_intent(Intent::CloseSafeKey);
                }
                Screen::DeleteConfirm { .. } => {
                    model.apply_ui_intent(Intent::CancelDeleteCharacter);
                }
                _ => {}
            }
            continue;
        }
        #[cfg(feature = "ui-preview")]
        if value["type"] == "uiPreview" {
            if let Some(scene) = value["scene"]
                .as_str()
                .filter(|s| crate::ui_preview::SCENES.contains(s))
            {
                preview.scene = Some(scene.to_owned());
            }
            continue;
        }
        if value["type"] == "insets" {
            host.ime_bottom = value["bottom"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_right = value["safeRight"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_top = value["safeTop"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_left = value["safeLeft"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_bottom = value["safeBottom"].as_u64().unwrap_or(0).min(8192) as f32;
            continue;
        }
        if value["type"] == "edit" {
            crate::text_input::edit_shell(
                &mut model,
                value["field"].as_str().unwrap_or(""),
                value["text"].as_str().unwrap_or(""),
            );
            if model.screen == Screen::InGame {
                if let Some(player) = player.as_deref_mut() {
                    forms.edit(
                        player,
                        value["field"].as_str().unwrap_or(""),
                        value["text"].as_str().unwrap_or(""),
                    );
                    crate::text_input::edit_player(
                        player,
                        value["field"].as_str().unwrap_or(""),
                        value["text"].as_str().unwrap_or(""),
                    );
                }
            }
            continue;
        }
        let phase = value["phase"].as_str().unwrap_or("");
        // Typed server data must not be recovered by parsing UI message text.
        // Reset on any non-world phase, including a map transition or reconnect.
        host.world = host_world_position(&value, model.screen);
        if matches!(phase, "DISCONNECTED" | "UNCONFIGURED" | "CONNECTING") {
            if let Some(effects) = effects.as_deref_mut() {
                discard_player_commands(effects);
            }
        }
        let message = value["message"]
            .as_str()
            .unwrap_or("Connection unavailable")
            .to_owned();
        match phase {
            "UNCONFIGURED" => {
                model.apply_gateway_event(Event::Disconnect { reason: None });
                model.screen = Screen::Login;
                model.notice = Some(ShellNotice::error(message));
            }
            "CONNECTING" => {
                model.screen = Screen::Connecting;
            }
            "READY" if model.login_request_in_flight => {
                model.apply_gateway_event(Event::LoginFailure { message });
            }
            "READY" => {
                model.apply_gateway_event(Event::Connected);
            }
            "CHARACTERS" if model.login_request_in_flight => {
                let characters = value["characters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|row| {
                        Some(CharacterSummary::new(
                            i32::try_from(row["index"].as_i64()?).ok()?,
                            row["name"].as_str()?,
                            row["level"].as_u64().unwrap_or(0).min(u16::MAX as u64) as u16,
                            row["className"].as_str().unwrap_or("Unknown"),
                            row["genderName"].as_str().unwrap_or("Unknown"),
                        ))
                    })
                    .collect();
                let account = model.login.account.clone();
                model.apply_gateway_event(Event::LoginSuccess {
                    account,
                    characters,
                });
            }
            "CHARACTERS" if model.screen == Screen::StartingGame => {
                model.apply_gateway_event(Event::StartGameAck {
                    accepted: false,
                    reason: Some(message),
                });
            }
            "IN_GAME" if model.screen == Screen::StartingGame => {
                model.apply_gateway_event(Event::StartGameAck {
                    accepted: true,
                    reason: None,
                });
                // This round validates shell UI only. Do not replace the player screen
                // with a debug position label or claim a rendered gameplay scene.
                model.notice = Some(ShellNotice::info(message));
            }
            "DISCONNECTED" => {
                model.apply_gateway_event(Event::Disconnect {
                    reason: Some(message),
                });
                intents.drain().for_each(drop);
                OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
            }
            _ => {}
        }
        host.phase = phase.to_owned();
    }
}

// Android Back must follow the same modal priority and cancellation rules as
// desktop Escape, not clear parent windows and bypass the shared reducers.
fn shared_back_key(events: &mut Messages<bevy::input::keyboard::KeyboardInput>, window: Entity) {
    use bevy::input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    };
    for state in [ButtonState::Pressed, ButtonState::Released] {
        events.write(KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state,
            text: None,
            repeat: false,
            window,
        });
    }
}

fn forward_intents(
    mut model: ResMut<NativeShellModel>,
    host: Res<HostState>,
    mut intents: ResMut<NativeUiIntentQueue>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
) {
    for intent in intents.drain() {
        match intent {
            Intent::Login | Intent::SafeKeyEnter if host.phase == "READY" => {
                send(
                    json!({"type":"login","account":model.login.account,"password":model.login.password}),
                );
                model.login.clear_password();
            }
            Intent::Login | Intent::SafeKeyEnter => {
                model.apply_gateway_event(Event::LoginFailure {
                    message: "Gateway not connected. Configure test endpoint and retry.".into(),
                });
                model.login.clear_password();
            }
            Intent::StartGame => {
                if let Some(index) = model.selected_character_index {
                    send(json!({"type":"start","index":index}));
                }
            }
            Intent::Retry => send(json!({"type":"connect"})),
            Intent::Logout => {
                if let Some(effects) = effects.as_deref_mut() {
                    discard_player_commands(effects);
                }
                send(json!({"type":"disconnect"}));
            }
            _ => {
                model.apply_gateway_event(Event::OperationFailure {
                    message: "This account operation is not wired in the Android UI milestone."
                        .into(),
                });
            }
        }
    }
}

// Only unsent shared Gateway effects are invalidated. Local option persistence
// and application effects must survive; this is not a server rollback/receipt.
fn discard_player_commands(effects: &mut mir2_client_bevy::crystal_ui::overlays::UiEffectQueue) {
    let mut discarded = 0;
    for effect in effects.drain() {
        if matches!(effect, mir2_ui_core::effect::UiEffect::GatewayCommand(_)) {
            discarded += 1;
        } else {
            effects.push(effect);
        }
    }
    if discarded > 0 {
        debug!("Discarded {discarded} unsent shared player commands at Android session boundary");
        #[cfg(feature = "ui-preview")]
        info!("ANDROID_UI_PREVIEW_DISCARD count={discarded} unsent shared commands");
    }
}

fn discard_inactive_player_commands(
    model: Res<NativeShellModel>,
    windows: Query<&Window>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
) {
    let active =
        model.screen == Screen::InGame && windows.single().is_ok_and(|window| window.focused);
    if !active {
        if let Some(effects) = effects.as_deref_mut() {
            discard_player_commands(effects);
        }
    }
}

fn keyboard(
    model: Res<NativeShellModel>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    forms: crate::form_input::FormInput,
    editor_touch: Res<EditorTouch>,
    interactions: Query<(&Interaction, &CrystalLoginAction), Changed<Interaction>>,
    extra_fields: Query<
        (
            &Interaction,
            &mir2_client_bevy::native_shell_ui::NativeShellField,
        ),
        Changed<Interaction>,
    >,
    mut was_editing: Local<bool>,
    mut privacy: Local<bool>,
    mut last_field: Local<Option<String>>,
) {
    let sensitive = !model.login.account.is_empty()
        || !model.login.password.is_empty()
        || (model.screen == Screen::InGame && forms.field(&player).is_some_and(|v| v.2))
        || matches!(model.screen, Screen::ChangePassword | Screen::SafeKey);
    if *privacy != sensitive {
        send(json!({"type":"privacy","secure":sensitive}));
        *privacy = sensitive;
    }
    let field = crate::text_input::shell_field(&model).or_else(|| {
        (model.screen == Screen::InGame)
            .then(|| {
                forms
                    .field(&player)
                    .or_else(|| crate::text_input::player_field(&player))
            })
            .flatten()
    });
    let field_name = field.map(|v| v.0.to_owned());
    let pressed = editor_touch.0
        || interactions.iter().any(|(interaction, action)| {
            *interaction == Interaction::Pressed
                && matches!(
                    action,
                    CrystalLoginAction::FocusAccount | CrystalLoginAction::FocusPassword
                )
        })
        || extra_fields
            .iter()
            .any(|(interaction, _)| *interaction == Interaction::Pressed)
        || (model.screen == Screen::InGame && field_name.is_some() && field_name != *last_field);
    if pressed {
        if let Some((field, text, password)) = field {
            send(
                json!({"type":"keyboard","field":field,"text":text,"password":password,"numeric":field.ends_with("amount"),"multiline":crate::text_input::is_multiline_editor(field)}),
            );
            *was_editing = true;
        } else if *was_editing {
            send(json!({"type":"hideKeyboard"}));
            *was_editing = false;
        }
    } else if field.is_none() && *was_editing {
        send(json!({"type":"hideKeyboard"}));
        *was_editing = false;
    }
    *last_field = field_name;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_world_requires_active_start_and_never_parses_notice_text() {
        let event = json!({"phase":"IN_GAME", "world": {
            "playerName":"ServerPlayer", "mapFileName":"0", "x":302, "y":634
        }});
        let position = host_world_position(&event, Screen::StartingGame).unwrap();
        assert_eq!((position.x, position.y), (302, 634));
        assert_eq!(position.map_file_name, "0");
        assert!(host_world_position(&event, Screen::Login).is_none());
        assert!(host_world_position(
            &json!({"phase":"IN_GAME", "message":"Server position: (302,634)"}),
            Screen::StartingGame
        )
        .is_none());
        for phase in ["DISCONNECTED", "CONNECTING", "STARTING", "CHARACTERS"] {
            let mut changed = event.clone();
            changed["phase"] = json!(phase);
            assert!(host_world_position(&changed, Screen::StartingGame).is_none());
        }
        for x in [json!(-1), json!(1.5), json!("302"), json!(4294967295u64)] {
            let mut invalid = event.clone();
            invalid["world"]["x"] = x;
            assert!(host_world_position(&invalid, Screen::StartingGame).is_none());
        }
    }

    #[test]
    fn belt_edge_fits_side_gutter_and_falls_back_without_overlap() {
        let fit = CrystalStageTransform::fit(891.0, 411.0);
        for vertical in [false, true] {
            let frame = mir2_client_bevy::crystal_ui::hud::belt_frame_rect(vertical);
            let origin = belt_edge_origin(fit, 411.0, 0.0, 12.0, vertical).unwrap();
            assert!(((origin.x + frame.left) * fit.scale - 8.0).abs() < 0.001);
            assert!(((origin.y + frame.top + frame.height) * fit.scale - 391.0).abs() < 0.001);
            assert!((origin.x + frame.left + frame.width) * fit.scale < fit.offset_x);
        }
        assert!(belt_edge_origin(
            CrystalStageTransform::fit(1024.0, 768.0),
            768.0,
            0.0,
            0.0,
            false
        )
        .is_none());
        assert!(belt_edge_origin(fit, 411.0, 100.0, 0.0, false).is_none());
    }

    #[test]
    fn minimap_anchor_preserves_scale_and_safe_edge_across_aspects() {
        for (width, height, dpi, right, top) in [
            (1024.0, 768.0, 1.0, 0.0, 0.0),
            (891.0, 411.0, 2.625, 63.0, 24.0),
            (1280.0, 720.0, 1.0, 40.0, 32.0),
        ] {
            let fit = CrystalStageTransform::fit(width, height);
            let origin = minimap_edge_origin(width, dpi, fit.scale, right, top);
            assert!(((origin.x + 1024.0) * fit.scale - (width - right / dpi - 8.0)).abs() < 0.001);
            assert!((origin.y * fit.scale - (top / dpi + 8.0)).abs() < 0.001);
            // A translated group moves both visual and button geometry. No
            // independent window-cursor rewrite is applied to centered dialogs.
            let button_x = 903.0;
            let screen_x = (origin.x + button_x) * fit.scale;
            assert!((screen_x / fit.scale - origin.x - button_x).abs() < 0.001);
            for dialog_top in [fit.offset_y / fit.scale, -180.0] {
                let group_top = origin.y - dialog_top;
                assert!((dialog_top + group_top - origin.y).abs() < 0.001);
            }
        }
    }

    #[test]
    fn shared_session_reset_clears_android_player_intents_without_reusing_storage_ids() {
        use mir2_client_bevy::{
            crystal_ui::overlays::{
                MailComposeUi, NativePlayerUiIntent, NativePlayerUiIntentQueue,
                NativePlayerUiState, UiEffectQueue,
            },
            pending_operations::{
                apply_overlay_session_reset, observe_native_session_boundary,
                AuthoritativeModelRevisions, InventoryOperationFeedback,
                NativeSessionBoundaryTracker, OverlayResetTracker, PendingOperations,
                SessionResetGameShopPreservation, SessionResetRevision,
            },
        };
        for destination in [Screen::Login, Screen::ConnectionLost] {
            let mut app = App::new();
            app.insert_resource(NativeShellModel {
                screen: Screen::InGame,
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<UiEffectQueue>()
            .init_resource::<MailComposeUi>()
            .init_resource::<PendingOperations>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<NativeSessionBoundaryTracker>()
            .init_resource::<OverlayResetTracker>()
            .init_resource::<AuthoritativeModelRevisions>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<SessionResetGameShopPreservation>()
            // Match the shared plugin: reset runs before input/boundary observation.
            .add_systems(
                Update,
                (apply_overlay_session_reset, observe_native_session_boundary).chain(),
            );
            app.update();
            let first_id = app.world_mut().resource_scope(
                |world, mut queue: Mut<NativePlayerUiIntentQueue>| {
                    let mut pending = world.resource_mut::<PendingOperations>();
                    assert!(queue.push_storage_pending_intent(&mut pending, true, 7, 0, 1));
                    let queued = queue.drain_intents();
                    let NativePlayerUiIntent::StoreItem { request_id, .. } = &queued[0] else {
                        panic!("expected storage intent")
                    };
                    let id = request_id.clone();
                    for intent in queued {
                        assert!(queue.push_intent(intent));
                    }
                    assert!(queue.push_intent(NativePlayerUiIntent::Chat {
                        message: "old-session".into()
                    }));
                    id
                },
            );
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .chat_draft = "old draft".into();
            app.world_mut().resource_mut::<NativeShellModel>().screen = destination;
            app.update();
            assert_eq!(app.world().resource::<SessionResetRevision>().0, 1);
            app.update();
            assert!(app
                .world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents()
                .is_empty());
            assert!(app.world().resource::<PendingOperations>().is_empty());
            assert!(app
                .world()
                .resource::<NativePlayerUiState>()
                .chat_draft
                .is_empty());
            let next_id = app.world_mut().resource_scope(
                |world, mut queue: Mut<NativePlayerUiIntentQueue>| {
                    let mut pending = world.resource_mut::<PendingOperations>();
                    assert!(queue.push_storage_pending_intent(&mut pending, true, 8, 0, 1));
                    let queued = queue.drain_intents();
                    let NativePlayerUiIntent::StoreItem { request_id, .. } = &queued[0] else {
                        panic!("expected storage intent")
                    };
                    request_id.clone()
                },
            );
            assert_ne!(
                first_id, next_id,
                "session cleanup must not reset request identity"
            );
        }
    }

    #[test]
    fn invalidation_discards_network_effects_but_retains_local_effects_in_order() {
        use mir2_client_bevy::crystal_ui::overlays::UiEffectQueue;
        use mir2_ui_core::effect::{GatewayCommand, UiEffect};
        let mut effects = UiEffectQueue::default();
        effects.push(UiEffect::ExitApplication);
        effects.push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
        effects.push(UiEffect::ExitApplication);
        discard_player_commands(&mut effects);
        discard_player_commands(&mut effects);
        assert_eq!(
            effects.drain(),
            vec![UiEffect::ExitApplication, UiEffect::ExitApplication]
        );
    }

    #[test]
    fn inactive_cleanup_runs_before_and_after_ui_producers_and_never_replays_on_resume() {
        use mir2_client_bevy::crystal_ui::overlays::UiEffectQueue;
        use mir2_ui_core::effect::{GatewayCommand, UiEffect};
        fn produce(mut effects: ResMut<UiEffectQueue>) {
            effects.push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
        }
        for (in_game, focused, has_window) in [
            (false, true, true),
            (true, false, true),
            (true, true, false),
            (true, true, true),
        ] {
            let mut app = App::new();
            app.insert_resource(NativeShellModel {
                screen: if in_game {
                    Screen::InGame
                } else {
                    Screen::Login
                },
                ..default()
            })
            .init_resource::<UiEffectQueue>()
            .add_systems(PreUpdate, discard_inactive_player_commands)
            .add_systems(Update, produce)
            .add_systems(PostUpdate, discard_inactive_player_commands);
            if has_window {
                app.world_mut().spawn(Window {
                    focused,
                    ..default()
                });
            }
            app.world_mut()
                .resource_mut::<UiEffectQueue>()
                .push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
            app.update();
            let active = in_game && focused && has_window;
            let first = app.world_mut().resource_mut::<UiEffectQueue>().drain();
            assert_eq!(first.len(), if active { 2 } else { 0 });
            app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::InGame;
            let world = app.world_mut();
            if has_window {
                world
                    .query::<&mut Window>()
                    .single_mut(world)
                    .unwrap()
                    .focused = true;
            } else {
                world.spawn(Window {
                    focused: true,
                    ..default()
                });
            }
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<UiEffectQueue>()
                    .drain()
                    .len(),
                1,
                "only the new frame's intent remains after resume"
            );
        }
    }

    #[test]
    fn guild_notice_keeps_all_eight_text_lines_above_ime() {
        assert_eq!(player_editor_bottom(Some("guild-notice"), 1.0), 333.0);
    }

    #[test]
    fn every_shared_amount_dialog_uses_its_modal_geometry_for_ime() {
        for field in ["inventory-amount", "guild-amount", "trade-amount"] {
            assert_eq!(player_editor_bottom(Some(field), 1.0), 446.0);
        }
        assert_eq!(player_editor_bottom(Some("chat"), 1.0), 750.0);
    }

    #[test]
    fn mail_ime_footer_reflows_and_restores_without_replacing_shared_buttons() {
        use mir2_client_bevy::crystal_ui::overlays::{MailComposeDetails, MailComposeFooter};
        let mut app = App::new();
        app.init_resource::<HostState>()
            .init_resource::<UiScale>()
            .add_systems(Update, fit_mail_composer);
        let details = app
            .world_mut()
            .spawn((MailComposeDetails, Node::default()))
            .id();
        let button = app
            .world_mut()
            .spawn((
                Button,
                Node {
                    top: px(408),
                    height: px(28),
                    ..default()
                },
            ))
            .id();
        let footer = app
            .world_mut()
            .spawn((MailComposeFooter, Node::default()))
            .add_child(button)
            .id();
        // Disabled Send has no Button component, but must retain the same size.
        let disabled = app
            .world_mut()
            .spawn(Node {
                top: px(408),
                height: px(28),
                ..default()
            })
            .id();
        app.world_mut().entity_mut(footer).add_child(disabled);
        for scale in [0.35, 0.535, 1.0, 2.0] {
            app.world_mut().resource_mut::<UiScale>().0 = scale;
            for editing in [true, false, true, false] {
                app.world_mut().resource_mut::<HostState>().ime_bottom =
                    if editing { 500.0 } else { 0.0 };
                app.update();
                let footer_node = app.world().get::<Node>(footer).unwrap();
                let button_node = app.world().get::<Node>(button).unwrap();
                assert_eq!(
                    app.world().get::<Node>(details).unwrap().display,
                    if editing {
                        Display::None
                    } else {
                        Display::Flex
                    }
                );
                assert_eq!(footer_node.top, px(if editing { -298.0 } else { 0.0 }));
                let height = if editing {
                    (44.0 / scale).max(28.0)
                } else {
                    28.0
                };
                assert_eq!(button_node.height, px(height));
                assert_eq!(
                    app.world().get::<Node>(disabled).unwrap().height,
                    px(height)
                );
                assert_eq!(button_node.top, px(408));
                if editing {
                    assert!(height * scale >= 44.0);
                    let bottom = player_editor_bottom(Some("mail-message"), scale);
                    assert!(((bottom - 115.0 - height) * scale - 8.0).abs() < 0.001);
                    assert_eq!(bottom, player_editor_bottom(Some("mail-recipient"), scale));
                }
            }
        }
    }

    #[test]
    fn android_back_is_one_shared_escape_press_release_pair() {
        let mut events = Messages::default();
        let window = Entity::PLACEHOLDER;
        shared_back_key(&mut events, window);
        let sent: Vec<_> = events.drain().collect();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].state, bevy::input::ButtonState::Pressed);
        assert_eq!(sent[1].state, bevy::input::ButtonState::Released);
        assert!(sent.iter().all(|event| event.key_code == KeyCode::Escape
            && event.text.is_none()
            && !event.repeat));
    }

    #[test]
    fn repeated_editor_tap_reopens_but_held_press_does_not_reset_ime() {
        let mut app = App::new();
        app.init_resource::<EditorTouch>()
            .add_systems(Update, remember_editor_touch);
        let field = app
            .world_mut()
            .spawn((
                Interaction::Pressed,
                mir2_client_bevy::crystal_ui::overlays::NativeTextInputTarget,
            ))
            .id();
        app.update();
        assert!(app.world().resource::<EditorTouch>().0);
        app.update();
        assert!(!app.world().resource::<EditorTouch>().0);
        *app.world_mut().get_mut::<Interaction>(field).unwrap() = Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(field).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().resource::<EditorTouch>().0);
    }

    #[test]
    fn host_drives_shared_shell_without_a_parallel_form_or_fake_world() {
        INBOX.lock().unwrap().clear();
        OUTBOX.lock().unwrap().clear();
        let mut app = App::new();
        #[cfg(feature = "ui-preview")]
        app.init_resource::<crate::ui_preview::PreviewRequest>();
        app.init_resource::<NativeShellModel>()
            .init_resource::<HostState>()
            .init_resource::<NativeUiIntentQueue>()
            .add_systems(Update, (receive, forward_intents).chain());
        INBOX.lock().unwrap().push_back(json!({"phase":"READY"}));
        app.update();
        let mut model = app.world_mut().resource_mut::<NativeShellModel>();
        assert_eq!(model.screen, Screen::Login);
        model.login.account = "ui-fixture".into();
        model.login.password = "not-a-real-password".into();
        assert!(model.apply_ui_intent(Intent::Login));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::Login);
        app.update();
        assert!(app
            .world()
            .resource::<NativeShellModel>()
            .login
            .password
            .is_empty());
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command["type"], "login");
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"CHARACTERS","characters":[
            {"index":7,"name":"Fixture","level":12,"className":"Wizard","genderName":"Female"}]}));
        app.update();
        let mut model = app.world_mut().resource_mut::<NativeShellModel>();
        assert_eq!(model.screen, Screen::CharacterSelect);
        assert_eq!(model.characters[0].class_name, "Wizard");
        assert_eq!(model.selected_character_index, Some(7));
        assert!(model.apply_ui_intent(Intent::StartGame));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::StartGame);
        app.update();
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command["index"], 7);
        INBOX.lock().unwrap().push_back(
            json!({"phase":"IN_GAME","message":"Server position available; scene pending"}),
        );
        app.update();
        assert_ne!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::InGame
        );
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"DISCONNECTED","message":"Backgrounded"}));
        app.update();
        let model = app.world().resource::<NativeShellModel>();
        assert_eq!(model.screen, Screen::ConnectionLost);
        assert!(model.characters.is_empty());
        assert!(OUTBOX.lock().unwrap().is_empty());
    }
}
