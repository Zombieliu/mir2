//! Headless regressions for `OptionDialog.SoundBar` / `MusicSoundBar`.

use super::*;

fn volume_app(scale: f32) -> (App, Entity) {
    let mut app = App::new();
    let mut window = Window::default();
    window.focused = true;
    window.resolution.set(1024.0 * scale, 768.0 * scale);
    let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<NativeShellModel>()
        .init_resource::<SessionResetRevision>()
        .init_resource::<options_volume::OptionsVolumeDrag>()
        .init_resource::<UiEffectQueue>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<CursorMoved>()
        .add_systems(Update, options_volume::process);
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Options;
    (app, entity)
}

fn physical_option_point(channel: options_volume::VolumeChannel, x: f32, scale: f32) -> Vec2 {
    let bar = options_volume::rect(channel);
    Vec2::new(
        (CRYSTAL_OPTIONS_PANEL_RECT.left + bar.left + x) * scale,
        (CRYSTAL_OPTIONS_PANEL_RECT.top + bar.top + bar.height * 0.5) * scale,
    )
}

fn move_cursor(app: &mut App, window: Entity, position: Vec2) {
    app.world_mut()
        .get_mut::<Window>(window)
        .expect("primary window")
        .set_cursor_position(Some(position));
    app.world_mut().write_message(CursorMoved {
        window,
        position,
        delta: None,
    });
}

#[test]
fn source_volume_mapping_preserves_min_mid_and_max_positions() {
    assert_eq!(options_volume::volume_from_x(options_volume::VolumeChannel::Sound, 159.0), 0);
    assert_eq!(options_volume::volume_from_x(options_volume::VolumeChannel::Sound, 197.0), 50);
    assert_eq!(options_volume::volume_from_x(options_volume::VolumeChannel::Sound, 235.0), 100);
    assert_eq!(options_volume::volume_from_x(options_volume::VolumeChannel::Music, 2_000.0), 100);
    assert_eq!(options_volume::volume_from_x(options_volume::VolumeChannel::Music, -1.0), 0);
}

#[test]
fn scaled_sound_bar_captures_click_position_then_held_drag_without_duplicate_actions() {
    let scale = 2.0;
    let (mut app, window) = volume_app(scale);

    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Sound, 0.0, scale),
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.sound_volume, 0);
    assert_eq!(
        app.world().resource::<options_volume::OptionsVolumeDrag>().captured(),
        Some(options_volume::VolumeChannel::Sound)
    );

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Sound, 38.0, scale),
    );
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.sound_volume, 50);

    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Sound, 76.0, scale),
    );
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.sound_volume, 100);

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_none());
    assert!(app.world().resource::<NativePlayerUiState>().menu_pointer_consumed);

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_released(MouseButton::Left);
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Sound, 0.0, scale),
    );
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.sound_volume, 100);

    let effect_count = app
        .world_mut()
        .resource_mut::<UiEffectQueue>()
        .drain()
        .into_iter()
        .filter(|effect| matches!(effect, mir2_ui_core::effect::UiEffect::ApplyAudioSettings { .. }))
        .count();
    assert_eq!(effect_count, 3, "each distinct captured position changes audio once");
}

#[test]
fn batched_press_and_move_captures_the_first_point_then_uses_the_last_held_point() {
    let (mut app, window) = volume_app(1.0);
    let press = physical_option_point(options_volume::VolumeChannel::Sound, 0.0, 1.0);
    let moved_outside_right = physical_option_point(options_volume::VolumeChannel::Sound, 76.0, 1.0);
    // The final point lies just beyond the source hit rectangle. It must not
    // erase the valid initial press before the bar has captured it.
    move_cursor(&mut app, window, press);
    move_cursor(&mut app, window, moved_outside_right);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().resource::<options_volume::OptionsVolumeDrag>().captured(),
        Some(options_volume::VolumeChannel::Sound)
    );
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.sound_volume, 100);
}

#[test]
fn volume_capture_cancels_for_covered_focus_and_session_boundaries() {
    let (mut app, window) = volume_app(1.0);
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Music, 20.0, 1.0),
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    let selected = app.world().resource::<NativePlayerUiState>().core.options.music_volume;
    assert_eq!(
        app.world().resource::<options_volume::OptionsVolumeDrag>().captured(),
        Some(options_volume::VolumeChannel::Music)
    );

    app.world_mut().resource_mut::<NativePlayerUiState>().keyboard.open = true;
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Music, 76.0, 1.0),
    );
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_none());
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.music_volume, selected);

    // The keyboard host keeps its close-frame latch after the visible dialog
    // is gone. A held click must not reach the reopened options bar then.
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.keyboard.open = false;
        state.keyboard.input_consumed = true;
    }
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_none());
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.music_volume, selected);

    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.release(MouseButton::Left);
        mouse.clear_just_pressed(MouseButton::Left);
    }
    app.world_mut().resource_mut::<NativePlayerUiState>().keyboard.input_consumed = false;
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Music, 0.0, 1.0),
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_none());
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.music_volume, selected);

    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.release(MouseButton::Left);
        mouse.clear_just_pressed(MouseButton::Left);
    }
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_released(MouseButton::Left);
    // The isolated interaction app does not schedule the real
    // `equipment_creature_host::process` frame boundary, which clears this
    // latch before pointer handlers. Model that boundary before a distinct
    // fresh click; the separate consumed-frame regression keeps the guard.
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .menu_pointer_consumed = false;
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Music, 38.0, 1.0),
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_some());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);
    app.world_mut()
        .resource_mut::<SessionResetRevision>()
        .0 += 1;
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_none());
}

#[test]
fn earlier_pointer_consumption_blocks_a_new_volume_capture() {
    let (mut app, window) = volume_app(1.0);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .menu_pointer_consumed = true;
    move_cursor(
        &mut app,
        window,
        physical_option_point(options_volume::VolumeChannel::Sound, 0.0, 1.0),
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app
        .world()
        .resource::<options_volume::OptionsVolumeDrag>()
        .captured()
        .is_none());
    assert_eq!(app.world().resource::<NativePlayerUiState>().core.options.sound_volume, 80);
}

#[test]
fn renderer_uses_source_size_tracks_and_source_thumb_positions_without_half_bar_buttons() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Options;
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.options.sound_volume = 50;
        state.core.options.music_volume = 100;
    }
    app.update();

    let world = app.world_mut();
    let mut tracks = world.query::<(&options_volume::OptionsVolumeTrack, &Node, &FocusPolicy)>();
    let mut found = tracks
        .iter(world)
        .map(|(track, node, policy)| (track.0, node.left, node.top, node.width, node.height, *policy))
        .collect::<Vec<_>>();
    found.sort_by_key(|(channel, ..)| match channel {
        options_volume::VolumeChannel::Sound => 0,
        options_volume::VolumeChannel::Music => 1,
    });
    assert_eq!(
        found,
        vec![
            (
                options_volume::VolumeChannel::Sound,
                Val::Px(159.0),
                Val::Px(225.0),
                Val::Px(76.0),
                Val::Px(19.0),
                FocusPolicy::Block,
            ),
            (
                options_volume::VolumeChannel::Music,
                Val::Px(159.0),
                Val::Px(251.0),
                Val::Px(76.0),
                Val::Px(19.0),
                FocusPolicy::Block,
            ),
        ]
    );
    let mut thumbs = world.query::<(&options_volume::OptionsVolumeThumb, &Node)>();
    let mut found = thumbs
        .iter(world)
        .map(|(thumb, node)| (thumb.0, node.left, node.top, node.width, node.height))
        .collect::<Vec<_>>();
    found.sort_by_key(|(channel, ..)| match channel {
        options_volume::VolumeChannel::Sound => 0,
        options_volume::VolumeChannel::Music => 1,
    });
    assert_eq!(
        found,
        vec![
            (options_volume::VolumeChannel::Sound, Val::Px(196.0), Val::Px(218.0), Val::Px(8.0), Val::Px(22.0)),
            (options_volume::VolumeChannel::Music, Val::Px(233.0), Val::Px(244.0), Val::Px(8.0), Val::Px(22.0)),
        ]
    );
    let mut images = world.query::<&ImageNode>();
    assert!(images.iter(world).any(|image| {
        image
            .image
            .path()
            .is_some_and(|path| path.to_string() == "original-ui/Prguse2/468.png")
    }));
    let mut buttons = world.query::<&OverlayButton>();
    assert!(!buttons.iter(world).any(|button| {
        matches!(
            button,
            OverlayButton::OptionsSoundVolumeDown
                | OverlayButton::OptionsSoundVolumeUp
                | OverlayButton::OptionsMusicVolumeDown
                | OverlayButton::OptionsMusicVolumeUp
        )
    }));
}
