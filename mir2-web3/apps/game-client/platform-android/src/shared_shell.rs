//! Android adapter for the SAME Crystal shell used by the Windows host.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::{login::CrystalLoginAction, CrystalStageTransform},
    native_shell::{
        CharacterSummary, LoginFocus, NativeGatewayEvent as Event, NativeShellModel,
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

pub struct AndroidSharedShellPlugin;
impl Plugin for AndroidSharedShellPlugin {
    fn build(&self, app: &mut App) {
        let mut model = NativeShellModel::default();
        model.screen = Screen::Login;
        model.notice = Some(ShellNotice::info(
            "Configure an approved test Gateway before connecting.",
        ));
        app.insert_resource(model)
            .init_resource::<HostState>()
            .add_plugins(Mir2NativeShellUiPlugin)
            .add_systems(PreUpdate, receive)
            .add_systems(PostUpdate, (fit_stage, forward_intents, keyboard).chain());
    }
}

fn fit_stage(
    windows: Query<&Window>,
    host: Res<HostState>,
    mut scale: ResMut<UiScale>,
    mut roots: Query<&mut Node, With<NativeShellRoot>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let fit = CrystalStageTransform::fit(window.width(), window.height());
    // Keep field/button size stable while typing. Pan the shared stage to keep
    // its login panel above the keyboard instead of shrinking it to a thumbnail.
    let available = (window.height() - host.ime_bottom / window.scale_factor()).max(1.0);
    let panel = mir2_client_bevy::crystal_ui::spec::login::PANEL.rect;
    let top = if host.ime_bottom > 0.0 {
        (available / (2.0 * fit.scale) - panel.top - panel.height * 0.5)
            .min(fit.offset_y / fit.scale)
    } else {
        fit.offset_y / fit.scale
    };
    scale.0 = fit.scale;
    for mut root in &mut roots {
        root.left = px(fit.offset_x / fit.scale);
        root.top = px(top);
    }
}

fn receive(
    mut model: ResMut<NativeShellModel>,
    mut host: ResMut<HostState>,
    mut intents: ResMut<NativeUiIntentQueue>,
) {
    let values: Vec<_> = INBOX
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect();
    for value in values {
        if value["type"] == "insets" {
            host.ime_bottom = value["bottom"].as_u64().unwrap_or(0).min(8192) as f32;
            continue;
        }
        if value["type"] == "edit" {
            // The invisible OS editor is only an input adapter; the shared model owns text.
            if model.screen == Screen::Login {
                let text = value["text"].as_str().unwrap_or("");
                match (value["field"].as_str(), model.login.focus) {
                    (Some("account"), LoginFocus::Account) => {
                        model.login.account =
                            text.chars().filter(|c| !c.is_control()).take(24).collect()
                    }
                    (Some("password"), LoginFocus::Password) => {
                        model.login.password =
                            text.chars().filter(|c| !c.is_control()).take(32).collect()
                    }
                    _ => {}
                }
            }
            continue;
        }
        let phase = value["phase"].as_str().unwrap_or("");
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

fn forward_intents(
    mut model: ResMut<NativeShellModel>,
    host: Res<HostState>,
    mut intents: ResMut<NativeUiIntentQueue>,
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
            Intent::Logout => send(json!({"type":"disconnect"})),
            _ => {
                model.apply_gateway_event(Event::OperationFailure {
                    message: "This account operation is not wired in the Android UI milestone."
                        .into(),
                });
            }
        }
    }
}

fn keyboard(
    model: Res<NativeShellModel>,
    interactions: Query<(&Interaction, &CrystalLoginAction), Changed<Interaction>>,
    mut was_editing: Local<bool>,
    mut privacy: Local<bool>,
) {
    let sensitive = !model.login.account.is_empty()
        || !model.login.password.is_empty()
        || matches!(model.screen, Screen::ChangePassword | Screen::SafeKey);
    if *privacy != sensitive {
        send(json!({"type":"privacy","secure":sensitive}));
        *privacy = sensitive;
    }
    let field = if model.screen == Screen::Login {
        match model.login.focus {
            LoginFocus::Account => Some(("account", &model.login.account)),
            LoginFocus::Password => Some(("password", &model.login.password)),
            _ => None,
        }
    } else {
        None
    };
    let pressed = interactions.iter().any(|(interaction, action)| {
        *interaction == Interaction::Pressed
            && matches!(
                action,
                CrystalLoginAction::FocusAccount | CrystalLoginAction::FocusPassword
            )
    });
    if pressed {
        if let Some((field, text)) = field {
            send(json!({"type":"keyboard","field":field,"text":text}));
            *was_editing = true;
        } else if *was_editing {
            send(json!({"type":"hideKeyboard"}));
            *was_editing = false;
        }
    } else if field.is_none() && *was_editing {
        send(json!({"type":"hideKeyboard"}));
        *was_editing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_drives_shared_shell_without_a_parallel_form_or_fake_world() {
        INBOX.lock().unwrap().clear();
        OUTBOX.lock().unwrap().clear();
        let mut app = App::new();
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
