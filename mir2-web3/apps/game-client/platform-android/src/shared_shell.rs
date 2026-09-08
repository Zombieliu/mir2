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
struct HostState {
    phase: String,
    ime_bottom: f32,
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
                    forward_intents,
                    discard_inactive_player_commands,
                    keyboard,
                )
                    .chain(),
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
    mut roots: Query<
        (&mut Node, Has<NativeShellRoot>),
        Or<(
            With<NativeShellRoot>,
            With<mir2_client_bevy::crystal_ui::hud::CrystalHudRoot>,
            With<mir2_client_bevy::crystal_ui::chat::CrystalChatRoot>,
            With<mir2_client_bevy::crystal_ui::notice::CrystalNoticeRoot>,
            With<mir2_client_bevy::crystal_ui::minimap::CrystalMiniMapRoot>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayRoot>,
            With<mir2_client_bevy::quest_ui::QuestUiRoot>,
        )>,
    >,
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
        let editor_bottom = player_editor_bottom(field.map(|field| field.0));
        (available / fit.scale - editor_bottom).min(fit.offset_y / fit.scale)
    } else if host.ime_bottom > 0.0 {
        (available / (2.0 * fit.scale) - panel.top - panel.height * 0.5)
            .min(fit.offset_y / fit.scale)
    } else {
        fit.offset_y / fit.scale
    };
    scale.0 = fit.scale;
    for (mut root, _is_shell) in &mut roots {
        root.left = px(fit.offset_x / fit.scale);
        root.top = px(top);
    }
}

fn player_editor_bottom(field: Option<&str>) -> f32 {
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
        // Mail editors are at the top of the source panel, not at the HUD.
        Some("mail-recipient" | "mail-message") => 0.0,
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
        assert_eq!(player_editor_bottom(Some("guild-notice")), 333.0);
    }

    #[test]
    fn every_shared_amount_dialog_uses_its_modal_geometry_for_ime() {
        for field in ["inventory-amount", "guild-amount", "trade-amount"] {
            assert_eq!(player_editor_bottom(Some(field)), 446.0);
        }
        assert_eq!(player_editor_bottom(Some("mail-message")), 0.0);
        assert_eq!(player_editor_bottom(Some("chat")), 750.0);
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
