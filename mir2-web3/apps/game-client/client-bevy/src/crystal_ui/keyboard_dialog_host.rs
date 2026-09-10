use super::super::*;
use super::*;
use std::path::PathBuf;

#[derive(Default, Resource)]
pub struct KeyboardHost {
    pub path: Option<PathBuf>,
    loaded: bool,
    pub notice: Option<String>,
    dimensions: std::collections::HashMap<(String, u16), Vec2>,
    thumb_drag: bool,
}

impl KeyboardHost {
    pub fn from_environment() -> Self {
        let path = std::env::var_os("MIR2_KEYBINDS_PATH")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(|p| PathBuf::from(p).join("mir2-web3").join("KeyBinds.json"))
            });
        Self {
            path,
            ..Default::default()
        }
    }
    pub fn save(&mut self, model: &mut KeyboardDialogUi) {
        let Some(path) = &self.path else {
            return;
        };
        let result = (|| -> Result<(), String> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let temp = path.with_extension("json.tmp");
            let data = model.to_json().map_err(|e| e.to_string())?;
            std::fs::write(&temp, data).map_err(|e| e.to_string())?;
            std::fs::rename(&temp, path).map_err(|e| e.to_string())
        })();
        match result {
            Ok(()) => model.dirty = false,
            Err(e) => self.notice = Some(format!("Could not save keyboard settings: {e}")),
        }
    }
}

pub fn modifiers(keys: &ButtonInput<KeyCode>) -> KeyModifiers {
    KeyModifiers {
        alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        ctrl: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
        shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        tilde: keys.pressed(KeyCode::Backquote),
    }
}

/// Fallback for synthetic key events/tests; real keyboard messages retain the
/// current layout's logical letter rather than assuming a US physical layout.
pub fn key_name(key: KeyCode) -> Option<String> {
    let debug = format!("{key:?}");
    if let Some(letter) = debug.strip_prefix("Key") {
        if letter.len() == 1 {
            return Some(letter.into());
        }
    }
    if let Some(n) = debug.strip_prefix("Digit") {
        return Some(format!("D{n}"));
    }
    if let Some(n) = debug.strip_prefix("Numpad") {
        if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() {
            return Some(format!("NumPad{n}"));
        }
    }
    if debug.starts_with('F')
        && debug[1..]
            .parse::<u8>()
            .is_ok_and(|v| (1..=24).contains(&v))
    {
        return Some(debug);
    }
    Some(
        match key {
            KeyCode::Enter | KeyCode::NumpadEnter => "Return",
            KeyCode::Escape => "Escape",
            KeyCode::Tab => "Tab",
            KeyCode::Space => "Space",
            KeyCode::Backspace => "Back",
            KeyCode::Delete => "Delete",
            KeyCode::Insert => "Insert",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "Prior",
            KeyCode::PageDown => "Next",
            KeyCode::ArrowUp => "Up",
            KeyCode::ArrowDown => "Down",
            KeyCode::ArrowLeft => "Left",
            KeyCode::ArrowRight => "Right",
            KeyCode::PrintScreen => "PrintScreen",
            KeyCode::Pause => "Pause",
            KeyCode::CapsLock => "Capital",
            KeyCode::NumLock => "NumLock",
            KeyCode::ScrollLock => "Scroll",
            KeyCode::ControlLeft | KeyCode::ControlRight => "ControlKey",
            KeyCode::ShiftLeft | KeyCode::ShiftRight => "ShiftKey",
            KeyCode::AltLeft | KeyCode::AltRight => "Menu",
            KeyCode::Backquote => "Oem8",
            KeyCode::Minus => "OemMinus",
            KeyCode::Equal => "Oemplus",
            KeyCode::BracketLeft => "OemOpenBrackets",
            KeyCode::BracketRight => "OemCloseBrackets",
            KeyCode::Backslash => "OemPipe",
            KeyCode::Semicolon => "OemSemicolon",
            KeyCode::Quote => "OemQuotes",
            KeyCode::Comma => "Oemcomma",
            KeyCode::Period => "OemPeriod",
            KeyCode::Slash => "OemQuestion",
            KeyCode::NumpadAdd => "Add",
            KeyCode::NumpadSubtract => "Subtract",
            KeyCode::NumpadMultiply => "Multiply",
            KeyCode::NumpadDivide => "Divide",
            KeyCode::NumpadDecimal => "Decimal",
            _ => return None,
        }
        .into(),
    )
}

fn observed_name(event: &KeyboardInput) -> Option<String> {
    if let bevy::input::keyboard::Key::Character(value) = &event.logical_key {
        if value.len() == 1 && value.as_bytes()[0].is_ascii_alphabetic() {
            return Some(value.to_uppercase());
        }
    }
    key_name(event.key_code)
}

pub fn triggered(model: &KeyboardDialogUi, keys: &ButtonInput<KeyCode>, function: &str) -> bool {
    let Some(bind) = model.bindings.iter().find(|b| b.function == function) else {
        return false;
    };
    if let Some(edges) = &model.key_edges {
        return edges.iter().any(|(name, mods, pressed)| *pressed && bind.matches(name, *mods));
    }
    keys.get_just_pressed().any(|key| {
        let name = model
            .logical_names
            .get(&format!("{key:?}"))
            .cloned()
            .or_else(|| key_name(*key));
        name.is_some_and(|name| bind.matches(&name, modifiers(keys)))
    })
}

pub fn released(model: &KeyboardDialogUi, keys: &ButtonInput<KeyCode>, function: &str) -> bool {
    let Some(bind) = model.bindings.iter().find(|b| b.function == function) else {
        return false;
    };
    if let Some(edges) = &model.key_edges {
        return edges.iter().any(|(name, mods, pressed)| !*pressed && bind.matches(name, *mods));
    }
    keys.get_just_released()
        .any(|key| key_name(*key).is_some_and(|name| bind.matches(&name, modifiers(keys))))
}

/// ButtonInput already contains the final frame state. Reverse this batch to
/// recover its initial state, then replay in order. Track physical left/right
/// modifiers independently; releasing one Ctrl must not release the other.
fn event_modifiers(keys: &ButtonInput<KeyCode>, events: &[KeyboardInput]) -> Vec<KeyModifiers> {
    let mut held = keys.clone();
    for event in events.iter().rev().filter(|event| !event.repeat) {
        match event.state {
            ButtonState::Pressed => held.release(event.key_code),
            ButtonState::Released => held.press(event.key_code),
        }
    }
    events.iter().map(|event| {
        match event.state {
            ButtonState::Pressed => held.press(event.key_code),
            ButtonState::Released => held.release(event.key_code),
        }
        modifiers(&held)
    }).collect()
}

pub(in super::super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<KeyboardHost>,
    shell: Option<Res<NativeShellModel>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut events: MessageReader<KeyboardInput>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    state.keyboard.input_consumed = false;
    state.keyboard.key_edges = None;
    if !host.loaded {
        host.loaded = true;
        if let Some(path) = &host.path {
            match std::fs::read_to_string(path) {
                Ok(text) => {
                    if let Err(e) = state.keyboard.load_json(&text) {
                        host.notice = Some(format!("Could not load keyboard settings: {e}"));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => host.notice = Some(format!("Could not load keyboard settings: {e}")),
            }
        }
    }
    let in_game = shell
        .as_ref()
        .is_some_and(|s| s.screen == NativeShellScreen::InGame);
    let focused = windows.single().is_ok_and(|w| w.focused);
    let Some(keys) = keys else {
        state.keyboard.spell_target_lock = false;
        events.clear();
        return;
    };
    let blocked = state.amount_modal_open() || state.trade_dialog.message.is_some();
    let raw_events: Vec<_> = events.read().cloned().collect();
    state.keyboard.key_edges = (!raw_events.is_empty()).then(Vec::new);
    let event_mods = event_modifiers(&keys, &raw_events);
    for (event, mods) in raw_events.iter().zip(event_mods) {
        let pressed = event.state == ButtonState::Pressed;
        if let Some(name) = observed_name(event) {
            if in_game && focused && !blocked && !event.repeat {
                if let Some(edges) = &mut state.keyboard.key_edges {
                    edges.push((name.clone(), mods, pressed));
                }
            }
            state.keyboard.observe_lock_key_event(&name);
            state
                .keyboard
                .logical_names
                .insert(format!("{:?}", event.key_code), name.clone());
            if in_game && focused && !blocked && host.notice.is_none() && pressed && !event.repeat {
                if state.keyboard.capture(&name, mods) {
                    state.keyboard.input_consumed = true;
                }
            }
        }
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) || !focused || blocked {
        state.keyboard.spell_target_lock = false;
        state.keyboard.end_drag();
        host.thumb_drag = false;
        return;
    }
    if host.notice.is_some() {
        state.keyboard.input_consumed = true;
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Escape) {
            host.notice = None;
        } else if let (Ok(window), Some(mouse)) = (windows.single(), mouse.as_ref()) {
            if mouse.just_pressed(MouseButton::Left)
                && help_cursor_logical(window)
                    .is_some_and(|p| p.x >= 644.0 && p.x < 720.0 && p.y >= 446.0 && p.y < 471.0)
            {
                host.notice = None;
            }
        }
        return;
    }
    if !state.keyboard.open {
        return;
    }
    let (Ok(window), Some(mouse)) = (windows.single(), mouse) else {
        return;
    };
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    let Some(position) = state.keyboard.position else {
        return;
    };
    let local = cursor - Vec2::from(position);
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(action) = view::hit_action(&state.keyboard, local, |l, i| {
            host.dimensions.get(&(l.into(), i)).copied()
        }) {
            state.keyboard.input_consumed = true;
            if state.keyboard.action(action) {
                host.save(&mut state.keyboard);
            }
            if action == KeyboardAction::Reset {
                host.notice = Some("Keyboard settings have been reset back to default.".into());
            }
        } else if local.x >= 491.0
            && local.x < 507.0
            && local.y >= state.keyboard.thumb_y() as f32
            && local.y < state.keyboard.thumb_y() as f32 + 20.0
        {
            host.thumb_drag = true;
        } else if local.y >= 0.0 && local.y < 28.0 && local.x >= 0.0 && local.x < 489.0 {
            state.keyboard.begin_drag(cursor.to_array());
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if host.thumb_drag {
            state.keyboard.drag_thumb(local.y);
        } else {
            state.keyboard.drag_to(cursor.to_array());
        }
    } else {
        host.thumb_drag = false;
        state.keyboard.end_drag();
    }
}

pub(in super::super) fn render_system(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    panels: Query<Entity, With<view::KeyboardPanel>>,
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<KeyboardHost>,
    shell: Option<Res<NativeShellModel>>,
    assets: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    mut handles: Local<std::collections::HashMap<(String, u16), Handle<Image>>>,
) {
    for p in &panels {
        commands.entity(p).despawn();
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame)
        || (!state.keyboard.open && host.notice.is_none())
    {
        return;
    }
    let (Ok(root), Some(assets), Some(images)) = (roots.single(), assets, images) else {
        return;
    };
    for (l, indices) in [
        ("Title", vec![119, 120, 121, 122, 200, 201, 202]),
        ("Prguse", vec![1346, 1347, 360]),
        (
            "Prguse2",
            vec![
                360, 361, 362, 197, 198, 199, 207, 208, 209, 205, 206, 190, 191, 192, 201, 202,
            ],
        ),
    ] {
        for i in indices {
            let handle = handles
                .entry((l.into(), i))
                .or_insert_with(|| assets.load(format!("original-ui/{l}/{i}.png")));
            if let Some(image) = images.get(handle) {
                host.dimensions.insert(
                    (l.into(), i),
                    Vec2::new(image.width() as f32, image.height() as f32),
                );
            }
        }
    }
    commands.entity(root).with_children(|parent| {
        view::render(
            parent,
            &assets,
            &mut state.keyboard,
            Vec2::new(1024.0, 768.0),
            |l, i| host.dimensions.get(&(l.into(), i)).copied(),
        );
        if let Some(notice) = &host.notice {
            parent
                .spawn((
                    view::KeyboardPanel,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(284.0),
                        top: Val::Px(289.0),
                        width: Val::Px(456.0),
                        height: Val::Px(190.0),
                        ..default()
                    },
                    FocusPolicy::Block,
                    GlobalZIndex(1000),
                ))
                .with_children(|p| {
                    spawn_overlay_frame(p, &assets, "original-ui/Prguse/360.png", 456.0, 190.0);
                    let spec = CrystalButtonSpec::new(
                        "Title",
                        200,
                        201,
                        202,
                        CrystalRect::new(360.0, 157.0, 76.0, 25.0),
                        76.0,
                        25.0,
                    );
                    spawn_crystal_image_button(
                        p,
                        &assets,
                        spec,
                        CrystalButtonAssetSet::from_spec(spec),
                        KeyboardNoticeOk,
                        false,
                        true,
                    );
                    overlay_text_at(
                        p,
                        notice,
                        CrystalRect::new(35.0, 35.0, 390.0, 110.0),
                        10.0,
                        Color::WHITE,
                    );
                });
        }
    });
}

#[derive(Component)]
struct KeyboardNoticeOk;

#[cfg(test)]
mod tests {
    use super::*;
    fn chord_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<NativePlayerUiState>()
            .init_resource::<KeyboardHost>()
            .insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..Default::default() })
            .add_systems(Update, process);
        let window = app.world_mut().spawn((Window { focused:true, ..Default::default() },PrimaryWindow)).id();
        (app,window)
    }
    fn chord_event(app: &mut App, window: Entity, key: KeyCode, pressed: bool, repeat: bool) {
        app.world_mut().write_message(KeyboardInput {
            key_code:key,
            logical_key:match key { KeyCode::F1=>bevy::input::keyboard::Key::F1, KeyCode::ShiftLeft=>bevy::input::keyboard::Key::Shift, _=>bevy::input::keyboard::Key::Control },
            state:if pressed {ButtonState::Pressed} else {ButtonState::Released},
            text:None,repeat,window,
        });
    }
    fn chord_banks(app: &App) -> (bool,bool) {
        let state=app.world().resource::<NativePlayerUiState>();
        let keys=app.world().resource::<ButtonInput<KeyCode>>();
        (triggered(&state.keyboard,keys,"Bar1Skill1"),triggered(&state.keyboard,keys,"Bar2Skill1"))
    }
    #[test]
    fn quick_control_chord_uses_keydown_modifiers_in_real_input_pipeline() {
        let (mut app,w)=chord_app();
        for (key,down) in [(KeyCode::ControlLeft,true),(KeyCode::F1,true),(KeyCode::F1,false),(KeyCode::ControlLeft,false)] {
            chord_event(&mut app,w,key,down,false);
        }
        app.update();
        assert_eq!(chord_banks(&app),(false,true));
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::ControlLeft));
        app.update();
        assert_eq!(chord_banks(&app),(false,false),"no replay next frame");
        chord_event(&mut app,w,KeyCode::F1,true,false);
        chord_event(&mut app,w,KeyCode::F1,false,false);
        chord_event(&mut app,w,KeyCode::ControlLeft,true,false);
        app.update();
        assert_eq!(chord_banks(&app),(true,false),"later Ctrl cannot modify earlier F1");
    }
    #[test]
    fn held_and_two_sided_control_releases_preserve_event_time_bank() {
        let (mut app,w)=chord_app();
        chord_event(&mut app,w,KeyCode::ControlRight,true,false);
        app.update();
        for (key,down) in [(KeyCode::ControlLeft,true),(KeyCode::ControlLeft,false),(KeyCode::F1,true),(KeyCode::F1,false),(KeyCode::ControlRight,false)] {
            chord_event(&mut app,w,key,down,false);
        }
        app.update();
        assert_eq!(chord_banks(&app),(false,true));
        chord_event(&mut app,w,KeyCode::F1,true,true);
        app.update();
        assert_eq!(chord_banks(&app),(false,false),"repeat does not invent a just-pressed binding");
    }
    #[test]
    fn chord_capture_consumes_gameplay_and_unfocused_edges_are_discarded() {
        let (mut app,w)=chord_app();
        {
            let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();
            state.keyboard.show();
            state.keyboard.action(KeyboardAction::Bind(0));
        }
        // Ctrl was held in the previous frame; the capture frame ends with release.
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
        chord_event(&mut app,w,KeyCode::F1,true,false);
        chord_event(&mut app,w,KeyCode::F1,false,false);
        chord_event(&mut app,w,KeyCode::ControlLeft,false,false);
        app.update();
        let state=app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.keyboard.bindings[0].key,"F1");
        assert_eq!(state.keyboard.bindings[0].ctrl,1);
        assert!(state.keyboard.input_consumed);
        assert!(state.blocks_gameplay_keys());
        app.world_mut().get_mut::<Window>(w).unwrap().focused=false;
        chord_event(&mut app,w,KeyCode::F1,true,false);
        app.update();
        assert_eq!(chord_banks(&app),(false,false));
    }

    #[test]
    fn capture_escape_is_consumed_and_unfocused_capture_is_ignored() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<KeyboardHost>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_message::<KeyboardInput>()
            .add_systems(Update, process);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    focused: true,
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.keyboard.show();
            state.keyboard.action(KeyboardAction::Bind(0));
        }
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: bevy::input::keyboard::Key::Escape,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        app.update();
        {
            let state = app.world().resource::<NativePlayerUiState>();
            assert_eq!(state.keyboard.bindings[0].key, "Escape");
            assert!(state.keyboard.input_consumed);
            assert!(state.blocks_gameplay_keys());
            assert!(state.keyboard.open);
        }
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .keyboard
            .action(KeyboardAction::Bind(0));
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: bevy::input::keyboard::Key::Character("a".into()),
            state: ButtonState::Pressed,
            text: Some("a".into()),
            repeat: false,
            window,
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .keyboard
                .bindings[0]
                .key,
            "Escape"
        );
    }
    #[test]
    fn remapped_binding_suppresses_original_key_and_obeys_modifiers() {
        let mut model = KeyboardDialogUi::default();
        let bind = model
            .bindings
            .iter_mut()
            .find(|b| b.function == "Inventory2")
            .unwrap();
        bind.key = "F4".into();
        bind.ctrl = 1;
        bind.alt = 0;
        bind.shift = 0;
        bind.tilde = 0;
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyI);
        assert!(!triggered(&model, &keys, "Inventory2"));
        keys.reset_all();
        keys.press(KeyCode::F4);
        assert!(!triggered(&model, &keys, "Inventory2"));
        keys.press(KeyCode::ControlLeft);
        assert!(triggered(&model, &keys, "Inventory2"));
        keys.press(KeyCode::ShiftLeft);
        assert!(!triggered(&model, &keys, "Inventory2"));
    }
    #[test]
    fn logical_keyboard_layout_and_numpad_remain_distinct() {
        let mut model = KeyboardDialogUi::default();
        model.logical_names.insert("KeyQ".into(), "I".into());
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyQ);
        assert!(triggered(&model, &keys, "Inventory2"));
        assert!(!triggered(&model, &keys, "Quests"));
        assert_eq!(key_name(KeyCode::Numpad1).as_deref(), Some("NumPad1"));
        assert_eq!(key_name(KeyCode::Digit1).as_deref(), Some("D1"));
    }
    #[test]
    fn disk_save_replaces_previous_file_and_loads_application_scoped_binding() {
        let dir = std::env::temp_dir().join(format!("mir2-keybind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bindings.json");
        let mut host = KeyboardHost {
            path: Some(path.clone()),
            ..Default::default()
        };
        let mut state = NativePlayerUiState::default();
        host.save(&mut state.keyboard);
        state.keyboard.bindings[0].key = "F24".into();
        state.keyboard.dirty = true;
        host.save(&mut state.keyboard);
        assert!(host.notice.is_none());
        assert!(!state.keyboard.dirty);
        let mut loaded = KeyboardDialogUi::default();
        loaded
            .load_json(&std::fs::read_to_string(&path).unwrap())
            .unwrap();
        assert_eq!(loaded.bindings[0].key, "F24");
        state.reset_session();
        assert_eq!(state.keyboard.bindings[0].key, "F24");
        assert!(!state.keyboard.open);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
