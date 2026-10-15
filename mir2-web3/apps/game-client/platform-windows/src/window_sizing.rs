//! Present Crystal's fixed stage at a chosen physical client size.
//!
//! Bevy 0.19 forwards WindowResizeConstraints to Winit as *OS logical* sizes,
//! even when WindowResolution has a scale-factor override. A 1024 minimum at
//! 150% DPI therefore enlarges the client area to 1536 physical pixels while
//! our pixel-authored HUD stays 1024 wide. Keep the OS override at one and scale
//! the fixed camera and *all* UI explicitly. Changing the override after Winit
//! creation would apply an additional physical-size conversion in Bevy 0.19.
use bevy::{
    camera::{RenderTarget, ScalingMode, Viewport},
    ecs::system::NonSendMarker,
    prelude::*,
    window::{
        PrimaryWindow, Window, WindowMode, WindowPosition, WindowRef, WindowResizeConstraints,
    },
};
use mir2_client_bevy::native_display::{
    preserve_original_stage, stage_viewport, NativeDisplaySettings, STAGE_HEIGHT, STAGE_WIDTH,
};

pub(crate) fn configure(window: &mut Window) {
    window.resizable = false;
    window.enabled_buttons.maximize = false;
    window.resolution.set_scale_factor_override(Some(1.0));
    window.resolution.set_physical_resolution(
        crate::session_config::DEFAULT_WINDOW_WIDTH,
        crate::session_config::DEFAULT_WINDOW_HEIGHT,
    );
    window.resize_constraints = WindowResizeConstraints {
        min_width: 1.0,
        min_height: 1.0,
        max_width: f32::INFINITY,
        max_height: f32::INFINITY,
    };
}

/// Run before UI focus so native hit testing receives the same scale as drawing.
/// OS work-area/DPI changes and selected resolutions share this single policy.
/// Only write Bevy components when values change, avoiding resize/layout loops.
pub(crate) fn sync_display_settings(
    mut settings: ResMut<NativeDisplaySettings>,
    mut ui_scale: ResMut<UiScale>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut cameras: Query<(&RenderTarget, &mut Camera, &mut Projection), With<Camera2d>>,
    _main_thread: NonSendMarker,
) {
    let Ok((entity, mut window)) = windows.single_mut() else {
        return;
    };
    let geometry = crate::native_display::geometry_for_window(entity);
    if let Some(geometry) = geometry {
        if settings.environment != geometry.environment {
            settings.set_environment(geometry.environment);
        }
    }
    if window.resolution.scale_factor_override() != Some(1.0) {
        window.resolution.set_scale_factor_override(Some(1.0));
    }
    let fullscreen = !matches!(window.mode, WindowMode::Windowed);
    if !fullscreen {
        let size = settings.size();
        if window.resolution.physical_size() != size {
            window.resolution.set_physical_resolution(size.x, size.y);
            if let Some(geometry) = geometry {
                window.position = WindowPosition::At(geometry.centered_outer_position(size));
            }
        }
    }
    // A full-screen client can be wide or portrait. The original stage stays
    // 4:3, with identical integer viewport bounds for camera, UI and tooltips.
    // The saved resolution continues to describe the next windowed client.
    let (viewport_position, viewport_size) = stage_viewport(window.resolution.physical_size());
    if viewport_size == UVec2::ZERO {
        return;
    }
    let scale = viewport_size.x as f32 / STAGE_WIDTH as f32;
    if ui_scale.0 != scale {
        ui_scale.0 = scale;
    }
    for (target, mut camera, mut projection) in &mut cameras {
        let is_stage_window = matches!(target, RenderTarget::Window(WindowRef::Primary))
            || matches!(target, RenderTarget::Window(WindowRef::Entity(target)) if *target == entity);
        if !is_stage_window {
            continue;
        }
        if fullscreen {
            let matches = camera.viewport.as_ref().is_some_and(|viewport| {
                viewport.physical_position == viewport_position
                    && viewport.physical_size == viewport_size
            });
            if !matches {
                camera.viewport = Some(Viewport {
                    physical_position: viewport_position,
                    physical_size: viewport_size,
                    ..default()
                });
            }
        } else if camera.viewport.is_some() {
            camera.viewport = None;
        }
        if let Projection::Orthographic(orthographic) = &*projection {
            if orthographic.scale != 1.0
                || !matches!(orthographic.scaling_mode,
                ScalingMode::Fixed { width, height } if width == STAGE_WIDTH as f32 && height == STAGE_HEIGHT as f32)
            {
                preserve_original_stage(&mut projection);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::{
        crystal_ui::metrics::CrystalStageTransform,
        native_display::{DisplayEnvironment, ResolutionChoice},
    };
    use winit::dpi::LogicalSize;

    #[test]
    fn fixed_os_logical_constraints_reproduce_the_reported_two_thirds_hud() {
        // These are the actual conversion semantics used by Winit, not the
        // client's CrystalStageTransform (which never caused this resize).
        let minimum = LogicalSize::new(1024.0, 768.0).to_physical::<u32>(1.5);
        assert_eq!((minimum.width, minimum.height), (1536, 1152));
        assert!((1024.0 / minimum.width as f32 - 2.0 / 3.0).abs() < 0.0001);
    }

    #[test]
    fn pixel_viewport_is_not_enlarged_by_os_dpi_constraints() {
        for dpi in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0] {
            let mut window = Window::default();
            window.resolution.set_scale_factor(dpi as f32);
            configure(&mut window);
            let constraints = window.resize_constraints.check_constraints();
            let minimum = LogicalSize::new(constraints.min_width, constraints.min_height)
                .to_physical::<u32>(dpi);
            assert!(minimum.width <= window.physical_width());
            assert!(minimum.height <= window.physical_height());
            assert!(!constraints.max_width.is_finite());
            assert!(!constraints.max_height.is_finite());
            assert_eq!(
                (window.physical_width(), window.physical_height()),
                (1024, 768)
            );
            assert_eq!((window.width(), window.height()), (1024.0, 768.0));
            assert!(!window.resizable && !window.enabled_buttons.maximize);
        }
    }

    #[test]
    fn selected_resolution_scales_world_ui_and_manual_input_as_one_stage() {
        let mut app = App::new();
        app.insert_resource(NativeDisplaySettings::new(
            ResolutionChoice::Auto,
            DisplayEnvironment {
                available: UVec2::new(4000, 2400),
                os_scale_factor: 1.0,
            },
        ))
        .insert_resource(UiScale(1.0))
        .add_systems(Update, sync_display_settings);
        let mut window = Window::default();
        configure(&mut window);
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        let camera = app.world_mut().spawn(Camera2d).id();
        let offscreen = app
            .world_mut()
            .spawn((
                Camera2d,
                RenderTarget::Image(Handle::<Image>::default().into()),
            ))
            .id();
        for choice in ResolutionChoice::ALL {
            app.world_mut()
                .resource_mut::<NativeDisplaySettings>()
                .select(choice);
            app.update();
            let window = app.world().get::<Window>(entity).unwrap();
            let expected = app.world().resource::<NativeDisplaySettings>().size();
            assert_eq!(window.resolution.physical_size(), expected);
            assert_eq!(window.resolution.scale_factor_override(), Some(1.0));
            let scale = app.world().resource::<UiScale>().0;
            assert_eq!(scale, expected.x as f32 / 1024.0);
            let Projection::Orthographic(projection) =
                app.world().get::<Projection>(camera).unwrap()
            else {
                panic!();
            };
            assert!(matches!(
                projection.scaling_mode,
                ScalingMode::Fixed {
                    width: 1024.0,
                    height: 768.0
                }
            ));
            assert_eq!(projection.scale, 1.0);
            let stage = CrystalStageTransform::fit_native(window.width(), window.height());
            for logical in [(0.0, 0.0), (433.0, 359.0), (1023.0, 767.0)] {
                let physical = stage.logical_to_physical(logical.0, logical.1);
                let inverse = stage.physical_to_logical(physical.0, physical.1);
                assert!(
                    (inverse.0 - logical.0).abs() < 0.001 && (inverse.1 - logical.1).abs() < 0.001
                );
                assert!((physical.0 - logical.0 * scale).abs() < 0.001);
            }
            app.update();
            assert_eq!(
                app.world()
                    .get::<Window>(entity)
                    .unwrap()
                    .resolution
                    .physical_size(),
                expected
            );
        }
        let Projection::Orthographic(projection) =
            app.world().get::<Projection>(offscreen).unwrap()
        else {
            panic!();
        };
        assert!(matches!(projection.scaling_mode, ScalingMode::WindowSize));
    }

    #[test]
    fn os_suggested_rectangle_is_corrected_without_changing_override_or_ui_scale() {
        let mut app = App::new();
        app.insert_resource(NativeDisplaySettings::new(
            ResolutionChoice::R1280x960,
            DisplayEnvironment {
                available: UVec2::new(1900, 1000),
                os_scale_factor: 1.5,
            },
        ))
        .insert_resource(UiScale(1.0))
        .add_systems(Update, sync_display_settings);
        let mut window = Window::default();
        configure(&mut window);
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        for factor in [1.0, 1.5, 2.0, 1.0] {
            let mut window = app.world_mut().get_mut::<Window>(entity).unwrap();
            window.resolution.set_scale_factor(factor);
            window
                .resolution
                .set_physical_resolution((1280.0 * factor) as u32, (960.0 * factor) as u32);
            drop(window);
            app.update();
            let window = app.world().get::<Window>(entity).unwrap();
            assert_eq!(window.resolution.physical_size(), UVec2::new(1280, 960));
            assert_eq!(window.resolution.scale_factor_override(), Some(1.0));
            assert_eq!(app.world().resource::<UiScale>().0, 1.25);
            app.update();
            assert_eq!(
                app.world()
                    .get::<Window>(entity)
                    .unwrap()
                    .resolution
                    .physical_size(),
                UVec2::new(1280, 960)
            );
        }
    }

    #[test]
    fn fullscreen_letterboxes_the_stage_and_restores_the_windowed_preference() {
        let mut app = App::new();
        app.insert_resource(NativeDisplaySettings::new(
            ResolutionChoice::R1280x960,
            DisplayEnvironment {
                available: UVec2::new(4000, 2400),
                os_scale_factor: 1.5,
            },
        ))
        .insert_resource(UiScale(1.0))
        .add_systems(Update, sync_display_settings);
        let mut window = Window::default();
        configure(&mut window);
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        let camera = app.world_mut().spawn(Camera2d).id();
        let explicit_camera = app
            .world_mut()
            .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(entity))))
            .id();
        let offscreen = app
            .world_mut()
            .spawn((
                Camera2d,
                RenderTarget::Image(Handle::<Image>::default().into()),
                Camera {
                    viewport: Some(Viewport {
                        physical_position: UVec2::new(7, 8),
                        physical_size: UVec2::new(128, 64),
                        ..default()
                    }),
                    ..default()
                },
            ))
            .id();
        app.update();
        for physical in [
            UVec2::new(1920, 1080),
            UVec2::new(2560, 1440),
            UVec2::new(3840, 2160),
            UVec2::new(2560, 1600),
            UVec2::new(1080, 1920),
        ] {
            {
                let mut window = app.world_mut().get_mut::<Window>(entity).unwrap();
                window.mode =
                    WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current);
                window
                    .resolution
                    .set_physical_resolution(physical.x, physical.y);
            }
            app.update();
            app.update();
            assert_eq!(
                app.world()
                    .get::<Window>(entity)
                    .unwrap()
                    .resolution
                    .physical_size(),
                physical,
                "full-screen monitor size must not be forced to the windowed preference"
            );
            let (origin, size) = stage_viewport(physical);
            let scale = size.x as f32 / 1024.0;
            assert_eq!(app.world().resource::<UiScale>().0, scale);
            assert_eq!(size.x * 3, size.y * 4);
            assert_eq!(
                app.world().resource::<NativeDisplaySettings>().choice,
                ResolutionChoice::R1280x960
            );
            for camera in [camera, explicit_camera] {
                let viewport = app
                    .world()
                    .get::<Camera>(camera)
                    .unwrap()
                    .viewport
                    .as_ref()
                    .unwrap();
                assert_eq!(
                    (viewport.physical_position, viewport.physical_size),
                    (origin, size)
                );
                let Projection::Orthographic(projection) =
                    app.world().get::<Projection>(camera).unwrap()
                else {
                    panic!()
                };
                assert!(matches!(
                    projection.scaling_mode,
                    ScalingMode::Fixed {
                        width: 1024.0,
                        height: 768.0
                    }
                ));
                assert_eq!(projection.scale, 1.0);
            }
            for logical in [
                Vec2::ZERO,
                Vec2::new(433.0, 359.0),
                Vec2::new(1023.0, 767.0),
            ] {
                let physical_hit = origin.as_vec2() + logical * scale;
                let stage = CrystalStageTransform::fit_native(physical.x as f32, physical.y as f32);
                assert_eq!(stage.scale, scale);
                assert_eq!(
                    (stage.offset_x, stage.offset_y),
                    (origin.x as f32, origin.y as f32)
                );
                let inverse = stage.physical_to_logical(physical_hit.x, physical_hit.y);
                assert!(Vec2::new(inverse.0, inverse.1).distance(logical) < 0.001);
                assert!(physical_hit.x >= origin.x as f32 && physical_hit.y >= origin.y as f32);
                assert!(
                    physical_hit.x < (origin.x + size.x) as f32
                        && physical_hit.y < (origin.y + size.y) as f32
                );
            }
            app.world_mut().get_mut::<Window>(entity).unwrap().mode = WindowMode::Windowed;
            app.update();
            assert_eq!(
                app.world()
                    .get::<Window>(entity)
                    .unwrap()
                    .resolution
                    .physical_size(),
                UVec2::new(1280, 960)
            );
            assert_eq!(app.world().resource::<UiScale>().0, 1.25);
            assert!(app
                .world()
                .get::<Camera>(camera)
                .unwrap()
                .viewport
                .is_none());
            assert!(app
                .world()
                .get::<Camera>(explicit_camera)
                .unwrap()
                .viewport
                .is_none());
        }
        let camera = app.world().get::<Camera>(offscreen).unwrap();
        let viewport = camera.viewport.as_ref().unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(7, 8));
        assert_eq!(viewport.physical_size, UVec2::new(128, 64));
        let Projection::Orthographic(projection) =
            app.world().get::<Projection>(offscreen).unwrap()
        else {
            panic!()
        };
        assert!(matches!(projection.scaling_mode, ScalingMode::WindowSize));
    }

    #[test]
    fn minimized_fullscreen_preserves_last_valid_scale_and_camera_viewport() {
        let mut app = App::new();
        app.insert_resource(NativeDisplaySettings::default())
            .insert_resource(UiScale(1.0))
            .add_systems(Update, sync_display_settings);
        let window = Window {
            mode: WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current),
            ..default()
        };
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        let camera = app.world_mut().spawn(Camera2d).id();
        app.update();
        let scale = app.world().resource::<UiScale>().0;
        let viewport = app
            .world()
            .get::<Camera>(camera)
            .unwrap()
            .viewport
            .clone()
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(entity)
            .unwrap()
            .resolution
            .set_physical_resolution(0, 0);
        app.update();
        assert_eq!(app.world().resource::<UiScale>().0, scale);
        let after = app
            .world()
            .get::<Camera>(camera)
            .unwrap()
            .viewport
            .as_ref()
            .unwrap();
        assert_eq!(after.physical_position, viewport.physical_position);
        assert_eq!(after.physical_size, viewport.physical_size);
    }

    /// Explicit native fixture: actual Winit resize, production resolution
    /// buttons, physical UI hit testing, world projection and window screenshots.
    /// No gateway, player account, install files or preferences are opened.
    #[test]
    #[ignore = "opens a short-lived native GPU window; run alone with a fresh MIR2_DISPLAY_NATIVE_OUTPUT directory"]
    fn real_winit_login_choices_resize_world_ui_and_mouse_together() {
        use bevy::{
            asset::AssetMetaCheck,
            input::InputSystems,
            render::pipelined_rendering::PipelinedRenderingPlugin,
            winit::{WinitPlugin, WinitSettings},
        };
        use mir2_client_bevy::{
            native_display::NativeDisplayUiPlugin,
            native_shell::{NativeShellModel, NativeShellScreen},
        };
        use std::{
            path::PathBuf,
            sync::{Arc, Mutex},
        };
        let output = PathBuf::from(
            std::env::var_os("MIR2_DISPLAY_NATIVE_OUTPUT")
                .expect("set a fresh native fixture output directory"),
        );
        assert!(
            output.is_absolute() && !output.exists(),
            "use a fresh output directory"
        );
        std::fs::create_dir(&output).unwrap();
        let report = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let mut window = Window {
            title: "Mir2 display verification".into(),
            ..default()
        };
        configure(&mut window);
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(bevy::image::ImagePlugin::default_nearest())
                .set(bevy::window::WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                .set(WinitPlugin {
                    run_on_any_thread: true,
                })
                .disable::<PipelinedRenderingPlugin>()
                .disable::<bevy::audio::AudioPlugin>()
                .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
        );
        app.insert_resource(WinitSettings::continuous())
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::Login,
                ..default()
            })
            .insert_resource(NativeDisplaySettings::default())
            .insert_resource(UiScale(1.0))
            .insert_resource(NativeSmoke {
                output: output.clone(),
                report: report.clone(),
                started: std::time::Instant::now(),
                phase: 0,
                wait: 20,
                index: 0,
                choices: Vec::new(),
                cursor: None,
                clicked: false,
                capture_done: false,
                windowed_size: None,
            })
            .add_plugins(NativeDisplayUiPlugin)
            .add_plugins(mir2_client_bevy::crystal_ui::widget::Mir2CrystalHintPlugin)
            .add_systems(Startup, native_smoke_scene)
            .add_systems(
                PreUpdate,
                native_smoke_drive
                    .after(InputSystems)
                    .before(sync_display_settings)
                    .before(bevy::ui::UiSystems::Focus),
            )
            .add_systems(
                PreUpdate,
                sync_display_settings.before(bevy::ui::UiSystems::Focus),
            )
            .add_systems(Update, native_smoke_hit);
        assert!(app.run().is_success());
        let rows = report.lock().unwrap();
        assert!(
            rows.iter().filter(|row| row["mode"] == "windowed").count() >= 3,
            "native work area must support two manual sizes and Auto"
        );
        assert!(rows.iter().any(|row| row["mode"] == "fullscreen"));
        assert!(rows.iter().any(|row| row["mode"] == "restored-windowed"));
        std::fs::write(
            output.join("native-display-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"passed": true, "cases": *rows}))
                .unwrap(),
        )
        .unwrap();
    }

    #[derive(Component)]
    struct NativeProbeButton;
    #[derive(Component)]
    struct NativeProbeSprite;
    #[derive(Resource)]
    struct NativeSmoke {
        output: std::path::PathBuf,
        report: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
        started: std::time::Instant,
        phase: u8,
        wait: u32,
        index: usize,
        choices: Vec<ResolutionChoice>,
        cursor: Option<Vec2>,
        clicked: bool,
        capture_done: bool,
        windowed_size: Option<UVec2>,
    }
    fn native_smoke_scene(mut commands: Commands) {
        commands.spawn((Camera2d, bevy::ui::IsDefaultUiCamera, Msaa::Off));
        commands.spawn((
            NativeProbeSprite,
            Sprite::from_color(Color::srgb(0.2, 0.8, 0.3), Vec2::new(64.0, 40.0)),
            Transform::from_xyz(-256.0, 128.0, 0.0),
        ));
        commands.spawn((
            NativeProbeButton,
            Button,
            mir2_client_bevy::crystal_ui::widget::CrystalHint::new("Display verification"),
            Node {
                position_type: PositionType::Absolute,
                left: px(480.0),
                top: px(300.0),
                width: px(64.0),
                height: px(40.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.8, 0.3, 0.2)),
            bevy::ui::FocusPolicy::Block,
        ));
    }
    fn native_smoke_hit(
        button: Query<&Interaction, (With<NativeProbeButton>, Changed<Interaction>)>,
        mut smoke: ResMut<NativeSmoke>,
    ) {
        if button
            .iter()
            .any(|interaction| *interaction == Interaction::Pressed)
        {
            smoke.clicked = true;
        }
    }
    fn native_smoke_drive(
        mut smoke: ResMut<NativeSmoke>,
        settings: Res<NativeDisplaySettings>,
        ui_scale: Res<UiScale>,
        mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
        choices: Query<(
            &mir2_client_bevy::native_display::DisplayChoice,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>,
        button: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<NativeProbeButton>>,
        hint: Query<
            (&ComputedNode, &bevy::ui::UiGlobalTransform, &Visibility),
            With<mir2_client_bevy::crystal_ui::widget::CrystalHintOverlayRoot>,
        >,
        sprites: Query<&GlobalTransform, With<NativeProbeSprite>>,
        cameras: Query<(&Camera, &GlobalTransform, &Projection), With<Camera2d>>,
        mut mouse: ResMut<ButtonInput<MouseButton>>,
        mut commands: Commands,
        mut exit: MessageWriter<bevy::app::AppExit>,
        _main_thread: NonSendMarker,
    ) {
        use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
        assert!(
            smoke.started.elapsed() < std::time::Duration::from_secs(45),
            "native resize fixture timed out at phase {}",
            smoke.phase
        );
        let Ok((entity, mut window)) = windows.single_mut() else {
            return;
        };
        if let Some(cursor) = smoke.cursor {
            window.set_cursor_position(Some(cursor));
        }
        if smoke.wait > 0 {
            smoke.wait -= 1;
            return;
        }
        match smoke.phase {
            0 => {
                if crate::native_display::geometry_for_window(entity).is_none() {
                    return;
                }
                smoke.choices = ResolutionChoice::ALL
                    .into_iter()
                    .filter(|choice| {
                        *choice != ResolutionChoice::Auto && settings.environment.supports(*choice)
                    })
                    .collect();
                smoke.choices.push(ResolutionChoice::Auto);
                smoke.phase = 1;
            }
            1 => {
                // Toggle is anchored left=12,bottom=12,width=328,min-height=50
                // in the original stage. Hit testing uses physical pixels.
                let cursor = Vec2::new(176.0, 731.0) * ui_scale.0;
                window.set_cursor_position(Some(cursor));
                smoke.cursor = Some(cursor);
                mouse.press(MouseButton::Left);
                smoke.phase = 2;
                smoke.wait = 2;
            }
            2 => {
                mouse.release(MouseButton::Left);
                let choice = smoke.choices[smoke.index];
                let (_, node, transform) = choices
                    .iter()
                    .find(|(marker, _, _)| marker.0 == choice)
                    .unwrap();
                assert!(
                    node.size().min_element() > 0.0,
                    "production display popup did not open"
                );
                smoke.cursor = Some(transform.translation);
                window.set_cursor_position(smoke.cursor);
                smoke.phase = 3;
                smoke.wait = 1;
            }
            3 => {
                mouse.press(MouseButton::Left);
                smoke.phase = 4;
                smoke.wait = 8;
            }
            4 => {
                mouse.release(MouseButton::Left);
                let selected = smoke.choices[smoke.index];
                assert_eq!(
                    settings.choice, selected,
                    "native UI did not select the pressed resolution"
                );
                let size = settings.size();
                let actual = bevy::winit::WINIT_WINDOWS
                    .with_borrow(|windows| windows.get_window(entity).unwrap().inner_size());
                assert_eq!(
                    (actual.width, actual.height),
                    (size.x, size.y),
                    "Winit must agree with the physical preference"
                );
                assert_eq!(window.resolution.physical_size(), size);
                assert_eq!(window.resolution.scale_factor_override(), Some(1.0));
                let (camera, transform, projection) = cameras.single().unwrap();
                assert_eq!(camera.physical_target_size(), Some(size));
                assert_eq!(camera.target_scaling_factor(), Some(1.0));
                let Projection::Orthographic(projection) = projection else {
                    panic!();
                };
                assert!(matches!(
                    projection.scaling_mode,
                    ScalingMode::Fixed {
                        width: 1024.0,
                        height: 768.0
                    }
                ));
                let sprite = camera
                    .world_to_viewport(transform, sprites.single().unwrap().translation())
                    .unwrap();
                assert!(sprite.distance(Vec2::new(256.0, 256.0) * ui_scale.0) < 0.1);
                let (node, transform) = button.single().unwrap();
                // Taffy rounds final UI edges to physical pixels at fractional
                // presentation factors (e.g. 1440/1024 = 1.40625).
                assert!(
                    node.size().distance(Vec2::new(64.0, 40.0) * ui_scale.0) <= 1.0,
                    "scaled UI size {:?} does not match {}",
                    node.size(),
                    ui_scale.0
                );
                assert!(
                    transform
                        .translation
                        .distance(Vec2::new(512.0, 320.0) * ui_scale.0)
                        <= 1.0
                );
                smoke.clicked = false;
                smoke.cursor = Some(transform.translation);
                window.set_cursor_position(smoke.cursor);
                smoke.phase = 5;
                smoke.wait = 1;
            }
            5 => {
                mouse.press(MouseButton::Left);
                smoke.phase = 6;
                smoke.wait = 1;
            }
            6 => {
                assert!(
                    smoke.clicked,
                    "physical mouse must hit the scaled native UI"
                );
                mouse.release(MouseButton::Left);
                smoke.phase = 9;
                smoke.wait = 3;
            }
            9 => {
                let (node, transform, visibility) = hint.single().unwrap();
                assert_eq!(
                    *visibility,
                    Visibility::Visible,
                    "production control tooltip must appear"
                );
                let min = transform.translation - node.size() / 2.0;
                assert!(
                    (min.y - 340.0 * ui_scale.0).abs() <= 1.0,
                    "tooltip cursor coordinates must be divided by UiScale exactly once"
                );
                assert!((min.x + node.size().x - 512.0 * ui_scale.0).abs() <= 1.0);
                let size = settings.size();
                let path = smoke
                    .output
                    .join(format!("native-{}.png", settings.choice.code()));
                assert!(!path.exists());
                smoke.capture_done = false;
                commands.spawn(Screenshot::primary_window()).observe(
                    move |captured: On<ScreenshotCaptured>, mut smoke: ResMut<NativeSmoke>| {
                        let image = captured
                            .image
                            .clone()
                            .try_into_dynamic()
                            .unwrap()
                            .to_rgba8();
                        assert_eq!(image.dimensions(), (size.x, size.y));
                        image.save(&path).unwrap();
                        smoke.capture_done = true;
                    },
                );
                smoke.phase = 7;
            }
            7 => {
                if !smoke.capture_done {
                    return;
                }
                let size = settings.size();
                smoke.report.lock().unwrap().push(serde_json::json!({
                    "mode": "windowed", "choice": settings.choice.code(), "physical": [size.x,size.y],
                    "uiScale": ui_scale.0, "cameraStage": [1024,768],
                    "worldAndUiMatch": true, "nativePhysicalHit": true, "tooltipMatchesCursor": true,
                }));
                smoke.index += 1;
                smoke.cursor = None;
                if smoke.index == smoke.choices.len() {
                    smoke.windowed_size = Some(size);
                    smoke.phase = 10;
                    smoke.wait = 2;
                } else {
                    smoke.phase = 1;
                    smoke.wait = 2;
                }
            }
            10 => {
                window.mode =
                    WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current);
                smoke.phase = 11;
                smoke.wait = 20;
            }
            11 | 17 => {
                let fullscreen = smoke.phase == 11;
                let actual = bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
                    let window = windows.get_window(entity).unwrap();
                    let inner = window.inner_size();
                    if fullscreen {
                        assert!(
                            window.fullscreen().is_some(),
                            "Winit must enter full screen"
                        );
                        assert_eq!(inner, window.current_monitor().unwrap().size());
                    } else {
                        assert!(
                            window.fullscreen().is_none(),
                            "Winit must leave full screen"
                        );
                    }
                    UVec2::new(inner.width, inner.height)
                });
                assert_eq!(window.resolution.physical_size(), actual);
                assert_eq!(window.resolution.scale_factor_override(), Some(1.0));
                if !fullscreen {
                    assert_eq!(actual, settings.size());
                    assert_eq!(
                        actual,
                        smoke.windowed_size.unwrap(),
                        "return to the previous windowed preference"
                    );
                }
                let (origin, size) = stage_viewport(actual);
                assert_eq!(ui_scale.0, size.x as f32 / 1024.0);
                let (camera, camera_transform, projection) = cameras.single().unwrap();
                assert_eq!(camera.physical_target_size(), Some(actual));
                assert_eq!(camera.physical_viewport_size(), Some(size));
                assert_eq!(
                    camera.physical_viewport_rect(),
                    Some(URect::from_corners(origin, origin + size))
                );
                assert_eq!(camera.target_scaling_factor(), Some(1.0));
                assert_eq!(camera.viewport.is_some(), fullscreen);
                let Projection::Orthographic(projection) = projection else {
                    panic!()
                };
                assert!(matches!(
                    projection.scaling_mode,
                    ScalingMode::Fixed {
                        width: 1024.0,
                        height: 768.0
                    }
                ));
                assert_eq!(projection.scale, 1.0);
                let sprite = camera
                    .world_to_viewport(camera_transform, sprites.single().unwrap().translation())
                    .unwrap();
                assert!(
                    sprite.distance(origin.as_vec2() + Vec2::new(256.0, 256.0) * ui_scale.0) < 0.1
                );
                let (node, transform) = button.single().unwrap();
                assert!(node.size().distance(Vec2::new(64.0, 40.0) * ui_scale.0) <= 1.0);
                assert!(
                    transform
                        .translation
                        .distance(Vec2::new(512.0, 320.0) * ui_scale.0)
                        <= 1.0
                );
                smoke.clicked = false;
                smoke.cursor = Some(origin.as_vec2() + transform.translation);
                let stage = CrystalStageTransform::fit_native(actual.x as f32, actual.y as f32);
                let physical = smoke.cursor.unwrap();
                let manual_hit = stage.physical_to_logical(physical.x, physical.y);
                assert!(
                    Vec2::new(manual_hit.0, manual_hit.1).distance(Vec2::new(512.0, 320.0)) <= 1.0
                );
                window.set_cursor_position(smoke.cursor);
                smoke.phase = if fullscreen { 12 } else { 18 };
                smoke.wait = 1;
            }
            12 | 18 => {
                mouse.press(MouseButton::Left);
                smoke.phase += 1;
                smoke.wait = 1;
            }
            13 | 19 => {
                assert!(
                    smoke.clicked,
                    "physical input must hit after changing the full-screen viewport"
                );
                mouse.release(MouseButton::Left);
                smoke.phase += 1;
                smoke.wait = 3;
            }
            14 | 20 => {
                let fullscreen = smoke.phase == 14;
                let (node, transform, visibility) = hint.single().unwrap();
                assert_eq!(*visibility, Visibility::Visible);
                let min = transform.translation - node.size() / 2.0;
                assert!(
                    (min.y - 340.0 * ui_scale.0).abs() <= 1.0,
                    "tooltip must remove the full-screen viewport origin before UiScale"
                );
                assert!((min.x + node.size().x - 512.0 * ui_scale.0).abs() <= 1.0);
                let size = window.resolution.physical_size();
                let path = smoke.output.join(if fullscreen {
                    "native-fullscreen.png"
                } else {
                    "native-restored-windowed.png"
                });
                assert!(!path.exists());
                smoke.capture_done = false;
                commands.spawn(Screenshot::primary_window()).observe(
                    move |captured: On<ScreenshotCaptured>, mut smoke: ResMut<NativeSmoke>| {
                        let image = captured
                            .image
                            .clone()
                            .try_into_dynamic()
                            .unwrap()
                            .to_rgba8();
                        assert_eq!(image.dimensions(), (size.x, size.y));
                        image.save(&path).unwrap();
                        smoke.capture_done = true;
                    },
                );
                smoke.phase += 1;
            }
            15 | 21 => {
                if !smoke.capture_done {
                    return;
                }
                let fullscreen = smoke.phase == 15;
                let physical = window.resolution.physical_size();
                let (origin, size) = stage_viewport(physical);
                smoke.report.lock().unwrap().push(serde_json::json!({
                    "mode": if fullscreen { "fullscreen" } else { "restored-windowed" },
                    "choice": settings.choice.code(), "physical": [physical.x,physical.y],
                    "viewportOrigin": [origin.x,origin.y], "viewportSize": [size.x,size.y],
                    "uiScale": ui_scale.0, "cameraStage": [1024,768],
                    "worldAndUiMatch": true, "nativePhysicalHit": true, "tooltipMatchesCursor": true,
                }));
                smoke.cursor = None;
                if fullscreen {
                    window.mode = WindowMode::Windowed;
                    smoke.phase = 17;
                    smoke.wait = 20;
                } else {
                    exit.write(bevy::app::AppExit::Success);
                    smoke.phase = 22;
                }
            }
            _ => {}
        }
    }
}
